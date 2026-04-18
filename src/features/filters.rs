use ahash::HashSet as AHashSet;
use compact_str::CompactString;
use regex::Regex;
use serde::{Deserialize, Deserializer};

/// `trxid` 过滤集合类型：使用 `ahash`（non-cryptographic SIMD 哈希），
/// 比标准 `SipHash` 快 2-3×，适合大量短字符串的热路径 `contains` 查询。
type TrxidSet = AHashSet<CompactString>;

/// 记录的元数据字段，传递给过滤器评估
#[derive(Debug)]
pub struct RecordMeta<'a> {
    pub trxid: &'a str,
    pub ip: &'a str,
    pub sess: &'a str,
    pub thrd: &'a str,
    pub user: &'a str,
    pub stmt: &'a str,
    pub app: &'a str,
    pub tag: Option<&'a str>,
}

fn vec_to_hashset<'de, D>(deserializer: D) -> Result<Option<TrxidSet>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Option<Vec<String>> = Option::deserialize(deserializer)?;
    // CompactString 对 ≤23 字节的字符串（trxid 通常是数字字符串）直接内联存储，
    // 消除堆分配，提升 HashSet 的 cache locality（bucket 内直接包含字符串数据）。
    Ok(v.map(|items| items.into_iter().map(CompactString::from).collect()))
}

fn vec_to_i64_hashset<'de, D>(deserializer: D) -> Result<Option<AHashSet<i64>>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Option<Vec<i64>> = Option::deserialize(deserializer)?;
    Ok(v.map(|items| items.into_iter().collect()))
}

/// 过滤器配置 (重构后)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct FiltersFeature {
    /// 是否启用过滤器
    pub enable: bool,
    /// 元数据过滤器 (记录级: 只要命中其中一个就保留该记录 - OR 逻辑)
    #[serde(flatten)]
    pub meta: MetaFilters,
    /// 指标过滤器 (事务级: 命中即保留整笔事务 - 需要预扫描)
    #[serde(default)]
    pub indicators: IndicatorFilters,
    /// SQL 内容过滤器 (事务级: 预扫描阶段匹配 SQL，保留整笔事务)
    #[serde(default)]
    pub sql: SqlFilters,
    /// SQL 记录级过滤器 (记录级: 在主扫描阶段对每条 DML 记录的 SQL 独立判断)
    #[serde(default)]
    pub record_sql: SqlFilters,
}

/// 元数据过滤器 (Record-level)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct MetaFilters {
    pub start_ts: Option<String>,
    pub end_ts: Option<String>,
    pub sess_ids: Option<Vec<String>>,
    pub thrd_ids: Option<Vec<String>>,
    pub usernames: Option<Vec<String>>,
    #[serde(default, deserialize_with = "vec_to_hashset")]
    pub trxids: Option<TrxidSet>,
    pub statements: Option<Vec<String>>,
    pub appnames: Option<Vec<String>>,
    pub client_ips: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

/// 指标过滤器 (Transaction-level)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct IndicatorFilters {
    /// 使用 `AHashSet<i64>` 代替 `Vec<i64>`，将 `matches()` 热路径中的
    /// `.contains()` 从 O(n) 降为 O(1)。
    #[serde(default, deserialize_with = "vec_to_i64_hashset")]
    pub exec_ids: Option<AHashSet<i64>>,
    pub min_runtime_ms: Option<u32>,
    pub min_row_count: Option<u32>,
}

/// SQL 过滤器 (未来扩展)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SqlFilters {
    pub include_patterns: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
}

impl FiltersFeature {
    /// 验证所有正则 pattern 格式合法。在 `Config::validate()` 中调用。
    pub fn validate_regexes(&self) -> crate::error::Result<()> {
        validate_pattern_list("features.filters.usernames", self.meta.usernames.as_deref())?;
        validate_pattern_list(
            "features.filters.client_ips",
            self.meta.client_ips.as_deref(),
        )?;
        validate_pattern_list("features.filters.sess_ids", self.meta.sess_ids.as_deref())?;
        validate_pattern_list("features.filters.thrd_ids", self.meta.thrd_ids.as_deref())?;
        validate_pattern_list(
            "features.filters.statements",
            self.meta.statements.as_deref(),
        )?;
        validate_pattern_list("features.filters.appnames", self.meta.appnames.as_deref())?;
        validate_pattern_list("features.filters.tags", self.meta.tags.as_deref())?;
        validate_pattern_list(
            "features.filters.record_sql.include_patterns",
            self.record_sql.include_patterns.as_deref(),
        )?;
        validate_pattern_list(
            "features.filters.record_sql.exclude_patterns",
            self.record_sql.exclude_patterns.as_deref(),
        )?;
        Ok(())
    }

    /// 检查是否配置了任何过滤器
    #[must_use]
    pub fn has_filters(&self) -> bool {
        if !self.enable {
            return false;
        }
        self.meta.start_ts.is_some()
            || self.meta.end_ts.is_some()
            || self.meta.has_filters()
            || self.indicators.has_filters()
            || self.sql.has_filters()
            || self.record_sql.has_filters()
    }

    /// 检查是否提供了需要预扫描的过滤器 (Transaction-level)
    #[must_use]
    pub fn has_transaction_filters(&self) -> bool {
        // 如果未开启过滤器功能，则不执行预扫描
        if !self.enable {
            return false;
        }
        self.indicators.has_filters() || self.sql.has_filters()
    }

    /// 检查记录是否应该被保留
    /// 逻辑：(满足时间过滤) AND ( (没有任何其他过滤) OR (满足任一元数据过滤) OR (属于被选中的事务) )
    #[must_use]
    pub fn should_keep(&self, ts: &str, meta: &RecordMeta) -> bool {
        // 1. 时间范围过滤 (AND 逻辑: 如果配置了时间，必须通过时间检查)
        if let Some(start) = &self.meta.start_ts {
            if ts < start.as_str() && !ts.starts_with(start.as_str()) {
                return false;
            }
        }
        if let Some(end) = &self.meta.end_ts {
            if ts > end.as_str() && !ts.starts_with(end.as_str()) {
                return false;
            }
        }

        // 2. 元数据过滤 (OR 逻辑: 在通过时间过滤的前提下，如果配置了元数据过滤，需命中其中之一)
        // 如果 meta.has_filters() 为 false，且通过了时间过滤，则保留。
        if !self.meta.has_filters() {
            return true;
        }

        self.meta.should_keep(meta)
    }

    /// 合并预扫描发现的事务 ID 到 `MetaFilters` 中，以便在正式扫描时直接通过 trxid 匹配保留整笔事务
    pub fn merge_found_trxids(&mut self, trxids: Vec<CompactString>) {
        if (!self.enable && !self.has_filters()) || trxids.is_empty() {
            return;
        }
        self.meta
            .trxids
            .get_or_insert_with(TrxidSet::default)
            .extend(trxids);
    }
}

impl MetaFilters {
    #[must_use]
    pub fn has_filters(&self) -> bool {
        self.trxids.as_ref().is_some_and(|v| !v.is_empty())
            || self.client_ips.as_ref().is_some_and(|v| !v.is_empty())
            || self.sess_ids.as_ref().is_some_and(|v| !v.is_empty())
            || self.thrd_ids.as_ref().is_some_and(|v| !v.is_empty())
            || self.usernames.as_ref().is_some_and(|v| !v.is_empty())
            || self.statements.as_ref().is_some_and(|v| !v.is_empty())
            || self.appnames.as_ref().is_some_and(|v| !v.is_empty())
            || self.tags.as_ref().is_some_and(|v| !v.is_empty())
    }

    #[must_use]
    pub fn should_keep(&self, meta: &RecordMeta) -> bool {
        // OR 逻辑：命中任何一个已定义的列表即保留 (前提是已通过时间过滤)
        // trxids 使用 HashSet<CompactString>，contains(&str) 通过 Borrow<str> 零分配查询
        Self::match_exact(self.trxids.as_ref(), meta.trxid)
            || Self::match_substring(self.client_ips.as_ref(), meta.ip)
            || Self::match_substring(self.sess_ids.as_ref(), meta.sess)
            || Self::match_substring(self.thrd_ids.as_ref(), meta.thrd)
            || Self::match_substring(self.usernames.as_ref(), meta.user)
            || Self::match_substring(self.statements.as_ref(), meta.stmt)
            || Self::match_substring(self.appnames.as_ref(), meta.app)
            || meta
                .tag
                .is_some_and(|t| Self::match_substring(self.tags.as_ref(), t))
    }

    /// O(1) 精确匹配，适用于高基数的 trxid 集合。
    /// `CompactString: Borrow<str>` 允许直接用 `&str` 查询 `TrxidSet`，无需分配。
    fn match_exact(set: Option<&TrxidSet>, val: &str) -> bool {
        set.is_some_and(|s| !s.is_empty() && s.contains(val))
    }

    /// O(n) 子串匹配，适用于小型过滤列表
    fn match_substring(list: Option<&Vec<String>>, val: &str) -> bool {
        list.is_some_and(|items| {
            !items.is_empty() && items.iter().any(|i| val.contains(i.as_str()))
        })
    }
}

/// 将正则字符串列表编译为 `Vec<Regex>`。None 或空列表返回 `Ok(None)`（未配置）。
/// 遇到非法正则时返回 `Err(bad_pattern)`。
fn compile_patterns(
    patterns: Option<&[String]>,
) -> std::result::Result<Option<Vec<Regex>>, String> {
    match patterns {
        None | Some([]) => Ok(None),
        Some(v) => {
            let compiled = v
                .iter()
                .map(|p| Regex::new(p).map_err(|_| p.clone()))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(Some(compiled))
        }
    }
}

/// None 表示"未配置，直接通过"；Some(patterns) 表示"任意一个匹配即满足"。
#[inline]
fn match_any_regex(patterns: Option<&[Regex]>, val: &str) -> bool {
    match patterns {
        None | Some([]) => true,
        Some(p) => p.iter().any(|re| re.is_match(val)),
    }
}

/// 验证一组正则字符串是否合法，任一失败则返回 `ConfigError::InvalidValue`。
fn validate_pattern_list(field: &str, patterns: Option<&[String]>) -> crate::error::Result<()> {
    let Some(list) = patterns else {
        return Ok(());
    };
    for pattern in list {
        Regex::new(pattern).map_err(|e| {
            crate::error::Error::Config(crate::error::ConfigError::InvalidValue {
                field: field.to_string(),
                value: pattern.clone(),
                reason: format!("invalid regex: {e}"),
            })
        })?;
    }
    Ok(())
}

/// 预编译后的元数据过滤器，在热路径中使用。由 `MetaFilters` 在启动时构造。
#[derive(Debug)]
pub struct CompiledMetaFilters {
    pub usernames: Option<Vec<Regex>>,
    pub client_ips: Option<Vec<Regex>>,
    pub sess_ids: Option<Vec<Regex>>,
    pub thrd_ids: Option<Vec<Regex>>,
    pub statements: Option<Vec<Regex>>,
    pub appnames: Option<Vec<Regex>>,
    pub tags: Option<Vec<Regex>>,
    pub trxids: Option<TrxidSet>,
}

impl CompiledMetaFilters {
    /// 从 `MetaFilters` 编译所有正则。须在 `Config::validate()` 之后调用——
    /// 验证已保证所有 pattern 合法，此处直接 expect。
    ///
    /// # Panics
    ///
    /// 如果 pattern 字符串不是合法正则（应在 `Config::validate()` 中提前拦截）。
    #[must_use]
    pub fn from_meta(meta: &MetaFilters) -> Self {
        Self {
            usernames: compile_patterns(meta.usernames.as_deref()).expect("regex validated"),
            client_ips: compile_patterns(meta.client_ips.as_deref()).expect("regex validated"),
            sess_ids: compile_patterns(meta.sess_ids.as_deref()).expect("regex validated"),
            thrd_ids: compile_patterns(meta.thrd_ids.as_deref()).expect("regex validated"),
            statements: compile_patterns(meta.statements.as_deref()).expect("regex validated"),
            appnames: compile_patterns(meta.appnames.as_deref()).expect("regex validated"),
            tags: compile_patterns(meta.tags.as_deref()).expect("regex validated"),
            trxids: meta.trxids.clone(),
        }
    }

    /// 是否有任何已编译的过滤条件（用于快路径跳过）。
    #[must_use]
    pub fn has_filters(&self) -> bool {
        self.usernames.is_some()
            || self.client_ips.is_some()
            || self.sess_ids.is_some()
            || self.thrd_ids.is_some()
            || self.statements.is_some()
            || self.appnames.is_some()
            || self.tags.is_some()
            || self.trxids.as_ref().is_some_and(|v| !v.is_empty())
    }

    /// AND 语义：所有已配置的字段都必须匹配记录才被保留（D-04）。
    /// 字段内 OR：同一字段列表中任意一个正则匹配即满足该字段（D-02）。
    #[inline]
    #[must_use]
    pub fn should_keep(&self, meta: &RecordMeta) -> bool {
        if !match_any_regex(self.usernames.as_deref(), meta.user) {
            return false;
        }
        if !match_any_regex(self.client_ips.as_deref(), meta.ip) {
            return false;
        }
        if !match_any_regex(self.sess_ids.as_deref(), meta.sess) {
            return false;
        }
        if !match_any_regex(self.thrd_ids.as_deref(), meta.thrd) {
            return false;
        }
        if !match_any_regex(self.statements.as_deref(), meta.stmt) {
            return false;
        }
        if !match_any_regex(self.appnames.as_deref(), meta.app) {
            return false;
        }
        // trxids：精确匹配（不用正则），参与 AND
        if let Some(trxids) = &self.trxids {
            if !trxids.is_empty() && !trxids.contains(meta.trxid) {
                return false;
            }
        }
        // tags：meta.tag 可能为 None，需要特殊处理
        if let Some(tag_patterns) = &self.tags {
            match meta.tag {
                Some(t) if !tag_patterns.iter().any(|re| re.is_match(t)) => return false,
                None if !tag_patterns.is_empty() => return false,
                _ => {}
            }
        }
        true
    }
}

/// 预编译后的 SQL 记录级过滤器（D-03）。
/// 仅用于 `record_sql`，事务级 `sql`（预扫描）保持字符串包含匹配。
#[derive(Debug)]
pub struct CompiledSqlFilters {
    pub include_patterns: Option<Vec<Regex>>,
    pub exclude_patterns: Option<Vec<Regex>>,
}

impl CompiledSqlFilters {
    /// 从 `SqlFilters` 编译正则。须在 `Config::validate()` 之后调用。
    ///
    /// # Panics
    ///
    /// 如果 pattern 字符串不是合法正则（应在 `Config::validate()` 中提前拦截）。
    #[must_use]
    pub fn from_sql_filters(sf: &SqlFilters) -> Self {
        Self {
            include_patterns: compile_patterns(sf.include_patterns.as_deref())
                .expect("regex validated"),
            exclude_patterns: compile_patterns(sf.exclude_patterns.as_deref())
                .expect("regex validated"),
        }
    }

    /// 是否有任何已编译的过滤条件。
    #[must_use]
    #[allow(dead_code)]
    pub fn has_filters(&self) -> bool {
        self.include_patterns.is_some() || self.exclude_patterns.is_some()
    }

    /// 判断 SQL 是否通过过滤：
    /// - include：必须命中其中之一（未配置 = 通过）
    /// - exclude：不能命中任何一个
    #[inline]
    #[must_use]
    pub fn matches(&self, sql: &str) -> bool {
        let include_ok = self
            .include_patterns
            .as_deref()
            .is_none_or(|p| p.is_empty() || p.iter().any(|re| re.is_match(sql)));
        if !include_ok {
            return false;
        }
        if let Some(excl) = &self.exclude_patterns {
            if excl.iter().any(|re| re.is_match(sql)) {
                return false;
            }
        }
        true
    }
}

impl IndicatorFilters {
    #[must_use]
    pub fn has_filters(&self) -> bool {
        self.exec_ids.as_ref().is_some_and(|v| !v.is_empty())
            || self.min_runtime_ms.is_some()
            || self.min_row_count.is_some()
    }

    #[must_use]
    pub fn matches(&self, exec_id: i64, runtime_ms: f32, row_count: i64) -> bool {
        if !self.has_filters() {
            return false;
        }

        if let Some(ids) = &self.exec_ids {
            if ids.contains(&exec_id) {
                return true;
            }
        }
        if let Some(min_t) = self.min_runtime_ms {
            if f64::from(runtime_ms) >= f64::from(min_t) {
                return true;
            }
        }
        if let Some(min_r) = self.min_row_count {
            if row_count >= i64::from(min_r) {
                return true;
            }
        }
        false
    }
}

impl SqlFilters {
    #[must_use]
    pub fn has_filters(&self) -> bool {
        self.include_patterns
            .as_ref()
            .is_some_and(|v| !v.is_empty())
            || self
                .exclude_patterns
                .as_ref()
                .is_some_and(|v| !v.is_empty())
    }

    #[must_use]
    pub fn matches(&self, sql: &str) -> bool {
        if !self.has_filters() {
            return false;
        }

        // 如果指定了包含模式，必须命中其中之一
        let include_match = if let Some(patterns) = &self.include_patterns {
            if patterns.is_empty() {
                true
            } else {
                patterns.iter().any(|p| sql.contains(p))
            }
        } else {
            true
        };

        if !include_match {
            return false;
        }

        // 如果指定了排除模式，不能命中任何一个
        if let Some(patterns) = &self.exclude_patterns {
            if patterns.iter().any(|p| sql.contains(p)) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_feature(enable: bool) -> FiltersFeature {
        FiltersFeature {
            enable,
            meta: MetaFilters::default(),
            indicators: IndicatorFilters::default(),
            sql: SqlFilters::default(),
            record_sql: SqlFilters::default(),
        }
    }

    // ── has_filters ────────────────────────────────────────────
    #[test]
    fn test_has_filters_disabled_returns_false() {
        let mut f = make_feature(false);
        f.meta.usernames = Some(vec!["USER".into()]);
        assert!(!f.has_filters());
    }

    #[test]
    fn test_has_filters_empty() {
        assert!(!make_feature(true).has_filters());
    }

    #[test]
    fn test_has_filters_with_username() {
        let mut f = make_feature(true);
        f.meta.usernames = Some(vec!["USER".into()]);
        assert!(f.has_filters());
    }

    #[test]
    fn test_has_filters_with_start_ts() {
        let mut f = make_feature(true);
        f.meta.start_ts = Some("2025-01-01".into());
        assert!(f.has_filters());
    }

    #[test]
    fn test_has_filters_with_indicator() {
        let mut f = make_feature(true);
        f.indicators.min_runtime_ms = Some(1000);
        assert!(f.has_filters());
    }

    // ── has_transaction_filters ────────────────────────────────
    #[test]
    fn test_has_transaction_filters_disabled() {
        let mut f = make_feature(false);
        f.indicators.min_runtime_ms = Some(1000);
        assert!(!f.has_transaction_filters());
    }

    #[test]
    fn test_has_transaction_filters_no_indicators() {
        let mut f = make_feature(true);
        f.meta.usernames = Some(vec!["USER".into()]);
        assert!(!f.has_transaction_filters());
    }

    #[test]
    fn test_has_transaction_filters_with_min_runtime() {
        let mut f = make_feature(true);
        f.indicators.min_runtime_ms = Some(500);
        assert!(f.has_transaction_filters());
    }

    #[test]
    fn test_has_transaction_filters_with_exec_ids() {
        let mut f = make_feature(true);
        f.indicators.exec_ids = Some([1_i64, 2, 3].into_iter().collect());
        assert!(f.has_transaction_filters());
    }

    fn m<'a>(trxid: &'a str, ip: &'a str, user: &'a str, tag: Option<&'a str>) -> RecordMeta<'a> {
        RecordMeta {
            trxid,
            ip,
            sess: "s",
            thrd: "t",
            user,
            stmt: "st",
            app: "a",
            tag,
        }
    }

    // ── should_keep: time range ────────────────────────────────
    #[test]
    fn test_should_keep_no_filters_passes_all() {
        let f = make_feature(true);
        assert!(f.should_keep("2025-01-15 10:00:00", &m("tx1", "1.2.3.4", "USER", None)));
    }

    #[test]
    fn test_should_keep_start_ts_before_record() {
        let mut f = make_feature(true);
        f.meta.start_ts = Some("2025-01-15 11:00:00".into());
        // record ts is before start → reject
        assert!(!f.should_keep("2025-01-15 10:00:00", &m("tx1", "1.2.3.4", "USER", None)));
    }

    #[test]
    fn test_should_keep_start_ts_equal_record() {
        let mut f = make_feature(true);
        f.meta.start_ts = Some("2025-01-15".into());
        // record ts starts with start → pass
        assert!(f.should_keep("2025-01-15 10:00:00", &m("tx1", "1.2.3.4", "USER", None)));
    }

    #[test]
    fn test_should_keep_end_ts_after_record() {
        let mut f = make_feature(true);
        f.meta.end_ts = Some("2025-01-15 09:00:00".into());
        // record ts is after end → reject
        assert!(!f.should_keep("2025-01-15 10:00:00", &m("tx1", "1.2.3.4", "USER", None)));
    }

    #[test]
    fn test_should_keep_meta_username_match() {
        let mut f = make_feature(true);
        f.meta.usernames = Some(vec!["USER".into()]);
        assert!(f.should_keep("2025-01-15 10:00:00", &m("tx1", "1.2.3.4", "USER", None)));
        assert!(!f.should_keep("2025-01-15 10:00:00", &m("tx1", "1.2.3.4", "OTHER", None)));
    }

    #[test]
    fn test_should_keep_meta_trxid_exact_match() {
        let mut f = make_feature(true);
        let mut set = TrxidSet::default();
        set.insert(CompactString::new("TX123"));
        f.meta.trxids = Some(set);
        assert!(f.should_keep("ts", &m("TX123", "ip", "u", None)));
        assert!(!f.should_keep("ts", &m("TX999", "ip", "u", None)));
    }

    #[test]
    fn test_should_keep_meta_tag_match() {
        let mut f = make_feature(true);
        f.meta.tags = Some(vec!["SEL".into()]);
        assert!(f.should_keep("ts", &m("tx", "ip", "u", Some("[SEL]"))));
        assert!(!f.should_keep("ts", &m("tx", "ip", "u", Some("[INS]"))));
        assert!(!f.should_keep("ts", &m("tx", "ip", "u", None)));
    }

    #[test]
    fn test_should_keep_meta_client_ip_substring() {
        let mut f = make_feature(true);
        f.meta.client_ips = Some(vec!["192.168".into()]);
        assert!(f.should_keep("ts", &m("tx", "192.168.1.1", "u", None)));
        assert!(!f.should_keep("ts", &m("tx", "10.0.0.1", "u", None)));
    }

    // ── merge_found_trxids ─────────────────────────────────────
    #[test]
    fn test_merge_found_trxids_empty_list() {
        let mut f = make_feature(true);
        f.meta.usernames = Some(vec!["USER".into()]);
        f.merge_found_trxids(vec![]);
        assert!(f.meta.trxids.is_none());
    }

    #[test]
    fn test_merge_found_trxids_adds_to_set() {
        let mut f = make_feature(true);
        f.meta.usernames = Some(vec!["USER".into()]);
        // "TX1".into() → CompactString (inline, no heap alloc)
        f.merge_found_trxids(vec!["TX1".into(), "TX2".into()]);
        let trxids = f.meta.trxids.unwrap();
        // contains(&str) works via CompactString: Borrow<str>
        assert!(trxids.contains("TX1"));
        assert!(trxids.contains("TX2"));
    }

    // ── IndicatorFilters ───────────────────────────────────────
    #[test]
    fn test_indicator_has_filters_empty() {
        assert!(!IndicatorFilters::default().has_filters());
    }

    #[test]
    fn test_indicator_matches_exec_id() {
        let f = IndicatorFilters {
            exec_ids: Some([42_i64].into_iter().collect()),
            min_runtime_ms: None,
            min_row_count: None,
        };
        assert!(f.matches(42, 0.0_f32, 0));
        assert!(!f.matches(99, 0.0_f32, 0));
    }

    #[test]
    fn test_indicator_matches_min_runtime() {
        let f = IndicatorFilters {
            exec_ids: None,
            min_runtime_ms: Some(1000),
            min_row_count: None,
        };
        assert!(f.matches(0, 1000.0_f32, 0));
        assert!(f.matches(0, 2000.0_f32, 0));
        assert!(!f.matches(0, 999.0_f32, 0));
    }

    #[test]
    fn test_indicator_matches_min_row_count() {
        let f = IndicatorFilters {
            exec_ids: None,
            min_runtime_ms: None,
            min_row_count: Some(100),
        };
        assert!(f.matches(0, 0.0_f32, 100));
        assert!(!f.matches(0, 0.0_f32, 99));
    }

    #[test]
    fn test_indicator_no_filters_always_false() {
        assert!(!IndicatorFilters::default().matches(1, 9999.0_f32, 9999));
    }

    // ── SqlFilters ─────────────────────────────────────────────
    #[test]
    fn test_sql_filters_empty() {
        assert!(!SqlFilters::default().has_filters());
        assert!(!SqlFilters::default().matches("SELECT 1"));
    }

    #[test]
    fn test_sql_filters_include_pattern() {
        let f = SqlFilters {
            include_patterns: Some(vec!["SELECT".into()]),
            exclude_patterns: None,
        };
        assert!(f.matches("SELECT * FROM t"));
        assert!(!f.matches("INSERT INTO t VALUES (1)"));
    }

    #[test]
    fn test_sql_filters_exclude_pattern() {
        let f = SqlFilters {
            include_patterns: None,
            exclude_patterns: Some(vec!["DROP".into()]),
        };
        assert!(f.matches("SELECT * FROM t"));
        assert!(!f.matches("DROP TABLE t"));
    }

    #[test]
    fn test_sql_filters_include_and_exclude() {
        let f = SqlFilters {
            include_patterns: Some(vec!["FROM t".into()]),
            exclude_patterns: Some(vec!["WHERE id=0".into()]),
        };
        assert!(f.matches("SELECT * FROM t WHERE id=1"));
        assert!(!f.matches("SELECT * FROM t WHERE id=0"));
        assert!(!f.matches("SELECT * FROM other"));
    }

    #[test]
    fn test_sql_filters_empty_include_patterns_with_exclude() {
        // include_patterns is Some but empty → line 248 path ("true" branch)
        // exclude_patterns is non-empty so has_filters() returns true
        let f = SqlFilters {
            include_patterns: Some(vec![]),
            exclude_patterns: Some(vec!["DROP".into()]),
        };
        // SQL doesn't match exclude → passes
        assert!(f.matches("SELECT 1"));
        // SQL matches exclude → filtered
        assert!(!f.matches("DROP TABLE t"));
    }

    // ── compile_patterns ───────────────────────────────────────
    #[test]
    fn test_compile_patterns_none() {
        let result = compile_patterns(None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_compile_patterns_empty() {
        let result = compile_patterns(Some(&[]));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_compile_patterns_valid() {
        let patterns = vec!["^admin.*".to_string()];
        let result = compile_patterns(Some(&patterns));
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert!(compiled.is_some());
        assert_eq!(compiled.unwrap().len(), 1);
    }

    #[test]
    fn test_compile_patterns_invalid() {
        let patterns = vec!["[invalid".to_string()];
        let result = compile_patterns(Some(&patterns));
        assert!(result.is_err());
    }

    // ── match_any_regex ────────────────────────────────────────
    #[test]
    fn test_match_any_regex_none_passes() {
        assert!(match_any_regex(None, "anything"));
    }

    #[test]
    fn test_match_any_regex_empty_passes() {
        assert!(match_any_regex(Some(&[]), "anything"));
    }

    #[test]
    fn test_match_any_regex_match() {
        use regex::Regex;
        let re = Regex::new("^admin").unwrap();
        assert!(match_any_regex(Some(&[re]), "admin_dba"));
    }

    #[test]
    fn test_match_any_regex_no_match() {
        use regex::Regex;
        let re = Regex::new("^admin").unwrap();
        assert!(!match_any_regex(Some(&[re]), "sys_admin"));
    }

    // ── CompiledMetaFilters ────────────────────────────────────
    fn make_compiled_meta(
        usernames: Option<Vec<String>>,
        client_ips: Option<Vec<String>>,
    ) -> CompiledMetaFilters {
        let meta = MetaFilters {
            usernames,
            client_ips,
            ..MetaFilters::default()
        };
        CompiledMetaFilters::from_meta(&meta)
    }

    #[test]
    fn test_compiled_meta_unconfigured_passes() {
        let compiled = make_compiled_meta(None, None);
        assert!(compiled.should_keep(&m("tx", "1.2.3.4", "any_user", None)));
    }

    #[test]
    fn test_compiled_meta_and_semantics() {
        let compiled = make_compiled_meta(
            Some(vec!["^admin".to_string()]),
            Some(vec!["^192\\.168".to_string()]),
        );
        // 两者都匹配 → 保留
        assert!(compiled.should_keep(&m("tx", "192.168.1.1", "admin_dba", None)));
        // 只有 username 匹配 → 拒绝
        assert!(!compiled.should_keep(&m("tx", "10.0.0.1", "admin_dba", None)));
        // 只有 ip 匹配 → 拒绝
        assert!(!compiled.should_keep(&m("tx", "192.168.1.1", "sys_user", None)));
    }

    #[test]
    fn test_compiled_meta_single_field_or() {
        let meta = MetaFilters {
            usernames: Some(vec!["^admin".to_string(), ".*_dba$".to_string()]),
            ..MetaFilters::default()
        };
        let compiled = CompiledMetaFilters::from_meta(&meta);
        assert!(compiled.should_keep(&m("tx", "ip", "admin_user", None)));
        assert!(compiled.should_keep(&m("tx", "ip", "sys_dba", None)));
        assert!(!compiled.should_keep(&m("tx", "ip", "regular_user", None)));
    }

    #[test]
    fn test_compiled_meta_tags_none_rejected() {
        let meta = MetaFilters {
            tags: Some(vec!["^SEL".to_string()]),
            ..MetaFilters::default()
        };
        let compiled = CompiledMetaFilters::from_meta(&meta);
        // tag 为 None 时，有 tag 过滤条件，拒绝
        assert!(!compiled.should_keep(&m("tx", "ip", "user", None)));
        // tag 匹配时通过
        assert!(compiled.should_keep(&m("tx", "ip", "user", Some("SELECT"))));
        // tag 不匹配时拒绝
        assert!(!compiled.should_keep(&m("tx", "ip", "user", Some("INSERT"))));
    }

    #[test]
    fn test_compiled_meta_trxids_and() {
        use compact_str::CompactString;
        let mut trxid_set = TrxidSet::default();
        trxid_set.insert(CompactString::from("TX123"));
        let meta = MetaFilters {
            usernames: Some(vec!["^admin".to_string()]),
            trxids: Some(trxid_set),
            ..MetaFilters::default()
        };
        let compiled = CompiledMetaFilters::from_meta(&meta);
        // 两者都满足 → 通过
        assert!(compiled.should_keep(&m("TX123", "ip", "admin_user", None)));
        // trxid 不匹配 → 拒绝（AND）
        assert!(!compiled.should_keep(&m("TX999", "ip", "admin_user", None)));
        // username 不匹配 → 拒绝（AND）
        assert!(!compiled.should_keep(&m("TX123", "ip", "other_user", None)));
    }

    // ── CompiledSqlFilters ─────────────────────────────────────
    #[test]
    fn test_compiled_sql_include_regex() {
        let sf = SqlFilters {
            include_patterns: Some(vec!["^SELECT".to_string()]),
            exclude_patterns: None,
        };
        let compiled = CompiledSqlFilters::from_sql_filters(&sf);
        assert!(compiled.matches("SELECT * FROM t"));
        assert!(!compiled.matches("INSERT INTO t VALUES (1)"));
    }

    #[test]
    fn test_compiled_sql_exclude_regex() {
        let sf = SqlFilters {
            include_patterns: None,
            exclude_patterns: Some(vec!["DROP".to_string()]),
        };
        let compiled = CompiledSqlFilters::from_sql_filters(&sf);
        assert!(compiled.matches("SELECT 1"));
        assert!(!compiled.matches("DROP TABLE t"));
    }

    #[test]
    fn test_filters_toml_deserialization_with_trxids_and_exec_ids() {
        // Exercises vec_to_hashset (lines 22-29) and vec_to_i64_hashset (lines 32-37)
        use crate::config::Config;
        let toml = r#"
[sqllog]
path = "sqllogs"
[features.filters]
enable = true
trxids = ["123", "456"]
[features.filters.indicators]
exec_ids = [1, 2, 3]
[exporter.csv]
file = "out.csv"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let filters = cfg.features.filters.unwrap();
        assert!(filters.meta.trxids.is_some());
        assert_eq!(filters.meta.trxids.unwrap().len(), 2);
        assert!(filters.indicators.exec_ids.is_some());
        assert_eq!(filters.indicators.exec_ids.unwrap().len(), 3);
    }
}

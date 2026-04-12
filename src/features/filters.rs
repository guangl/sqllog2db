use ahash::HashSet as AHashSet;
use compact_str::CompactString;
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
    /// SQL 内容过滤器 (事务级: 未来扩展)
    #[serde(default)]
    pub sql: SqlFilters,
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
    pub exec_ids: Option<Vec<i64>>,
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
        f.indicators.exec_ids = Some(vec![1, 2, 3]);
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
            exec_ids: Some(vec![42]),
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
}

use serde::{Deserialize, Deserializer};
use std::collections::HashSet;

fn vec_to_hashset<'de, D>(deserializer: D) -> Result<Option<HashSet<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Option<Vec<String>> = Option::deserialize(deserializer)?;
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
    pub trxids: Option<HashSet<String>>,
    pub statements: Option<Vec<String>>,
    pub appnames: Option<Vec<String>>,
    pub client_ips: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

/// 指标过滤器 (Transaction-level)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct IndicatorFilters {
    pub exec_ids: Option<Vec<i64>>,
    pub min_runtime_ms: Option<i64>,
    pub min_row_count: Option<i64>,
}

/// SQL 过滤器 (未来扩展)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SqlFilters {
    pub include_patterns: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
}

impl FiltersFeature {
    pub fn validate() {}

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
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn should_keep(
        &self,
        ts: &str,
        trxid: &str,
        ip: &str,
        sess: &str,
        thrd: &str,
        user: &str,
        stmt: &str,
        app: &str,
        tag: Option<&str>,
    ) -> bool {
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

        self.meta
            .should_keep(ts, trxid, ip, sess, thrd, user, stmt, app, tag)
    }

    /// 合并预扫描发现的事务 ID 到 `MetaFilters` 中，以便在正式扫描时直接通过 trxid 匹配保留整笔事务
    pub fn merge_found_trxids(&mut self, trxids: Vec<String>) {
        if (!self.enable && !self.has_filters()) || trxids.is_empty() {
            return;
        }
        self.meta
            .trxids
            .get_or_insert_with(HashSet::new)
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

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn should_keep(
        &self,
        _ts: &str,
        trxid: &str,
        ip: &str,
        sess: &str,
        thrd: &str,
        user: &str,
        stmt: &str,
        app: &str,
        tag: Option<&str>,
    ) -> bool {
        // OR 逻辑：命中任何一个已定义的列表即保留 (前提是已通过时间过滤)
        Self::match_exact(self.trxids.as_ref(), trxid)
            || Self::match_substring(self.client_ips.as_ref(), ip)
            || Self::match_substring(self.sess_ids.as_ref(), sess)
            || Self::match_substring(self.thrd_ids.as_ref(), thrd)
            || Self::match_substring(self.usernames.as_ref(), user)
            || Self::match_substring(self.statements.as_ref(), stmt)
            || Self::match_substring(self.appnames.as_ref(), app)
            || tag.is_some_and(|t| Self::match_substring(self.tags.as_ref(), t))
    }

    /// O(1) 精确匹配，适用于高基数的 trxid 集合
    fn match_exact(set: Option<&HashSet<String>>, val: &str) -> bool {
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
    pub fn matches(&self, exec_id: i64, runtime_ms: i64, row_count: i64) -> bool {
        if !self.has_filters() {
            return false;
        }

        if let Some(ids) = &self.exec_ids {
            if ids.contains(&exec_id) {
                return true;
            }
        }
        if let Some(min_t) = self.min_runtime_ms {
            if runtime_ms >= min_t {
                return true;
            }
        }
        if let Some(min_r) = self.min_row_count {
            if row_count >= min_r {
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

    // ── should_keep: time range ────────────────────────────────
    #[test]
    fn test_should_keep_no_filters_passes_all() {
        let f = make_feature(true);
        assert!(f.should_keep(
            "2025-01-15 10:00:00",
            "tx1",
            "1.2.3.4",
            "s1",
            "t1",
            "USER",
            "stmt",
            "app",
            None
        ));
    }

    #[test]
    fn test_should_keep_start_ts_before_record() {
        let mut f = make_feature(true);
        f.meta.start_ts = Some("2025-01-15 11:00:00".into());
        // record ts is before start → reject
        assert!(!f.should_keep(
            "2025-01-15 10:00:00",
            "tx1",
            "1.2.3.4",
            "s1",
            "t1",
            "USER",
            "stmt",
            "app",
            None
        ));
    }

    #[test]
    fn test_should_keep_start_ts_equal_record() {
        let mut f = make_feature(true);
        f.meta.start_ts = Some("2025-01-15".into());
        // record ts starts with start → pass
        assert!(f.should_keep(
            "2025-01-15 10:00:00",
            "tx1",
            "1.2.3.4",
            "s1",
            "t1",
            "USER",
            "stmt",
            "app",
            None
        ));
    }

    #[test]
    fn test_should_keep_end_ts_after_record() {
        let mut f = make_feature(true);
        f.meta.end_ts = Some("2025-01-15 09:00:00".into());
        // record ts is after end → reject
        assert!(!f.should_keep(
            "2025-01-15 10:00:00",
            "tx1",
            "1.2.3.4",
            "s1",
            "t1",
            "USER",
            "stmt",
            "app",
            None
        ));
    }

    #[test]
    fn test_should_keep_meta_username_match() {
        let mut f = make_feature(true);
        f.meta.usernames = Some(vec!["USER".into()]);
        assert!(f.should_keep(
            "2025-01-15 10:00:00",
            "tx1",
            "1.2.3.4",
            "s1",
            "t1",
            "USER",
            "stmt",
            "app",
            None
        ));
        assert!(!f.should_keep(
            "2025-01-15 10:00:00",
            "tx1",
            "1.2.3.4",
            "s1",
            "t1",
            "OTHER",
            "stmt",
            "app",
            None
        ));
    }

    #[test]
    fn test_should_keep_meta_trxid_exact_match() {
        let mut f = make_feature(true);
        let mut set = HashSet::new();
        set.insert("TX123".to_string());
        f.meta.trxids = Some(set);
        assert!(f.should_keep("ts", "TX123", "ip", "s", "t", "u", "st", "a", None));
        assert!(!f.should_keep("ts", "TX999", "ip", "s", "t", "u", "st", "a", None));
    }

    #[test]
    fn test_should_keep_meta_tag_match() {
        let mut f = make_feature(true);
        f.meta.tags = Some(vec!["SEL".into()]);
        assert!(f.should_keep("ts", "tx", "ip", "s", "t", "u", "st", "a", Some("[SEL]")));
        assert!(!f.should_keep("ts", "tx", "ip", "s", "t", "u", "st", "a", Some("[INS]")));
        assert!(!f.should_keep("ts", "tx", "ip", "s", "t", "u", "st", "a", None));
    }

    #[test]
    fn test_should_keep_meta_client_ip_substring() {
        let mut f = make_feature(true);
        f.meta.client_ips = Some(vec!["192.168".into()]);
        assert!(f.should_keep("ts", "tx", "192.168.1.1", "s", "t", "u", "st", "a", None));
        assert!(!f.should_keep("ts", "tx", "10.0.0.1", "s", "t", "u", "st", "a", None));
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
        f.merge_found_trxids(vec!["TX1".into(), "TX2".into()]);
        let trxids = f.meta.trxids.unwrap();
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
        assert!(f.matches(42, 0, 0));
        assert!(!f.matches(99, 0, 0));
    }

    #[test]
    fn test_indicator_matches_min_runtime() {
        let f = IndicatorFilters {
            exec_ids: None,
            min_runtime_ms: Some(1000),
            min_row_count: None,
        };
        assert!(f.matches(0, 1000, 0));
        assert!(f.matches(0, 2000, 0));
        assert!(!f.matches(0, 999, 0));
    }

    #[test]
    fn test_indicator_matches_min_row_count() {
        let f = IndicatorFilters {
            exec_ids: None,
            min_runtime_ms: None,
            min_row_count: Some(100),
        };
        assert!(f.matches(0, 0, 100));
        assert!(!f.matches(0, 0, 99));
    }

    #[test]
    fn test_indicator_no_filters_always_false() {
        assert!(!IndicatorFilters::default().matches(1, 9999, 9999));
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

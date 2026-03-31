use serde::Deserialize;
use std::collections::HashSet;

/// 过滤器配置 (重构后)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct FiltersFeature {
    /// 是否启用过滤器
    pub enable: bool,
    /// 元数据过滤器 (记录级: 只要命中其中一个就保留该记录 - OR 逻辑)
    #[serde(default)]
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
    pub sess_ids: Option<Vec<String>>,
    pub thrd_ids: Option<Vec<String>>,
    pub usernames: Option<Vec<String>>,
    pub trxids: Option<Vec<String>>,
    pub statements: Option<Vec<String>>,
    pub appnames: Option<Vec<String>>,
    pub client_ips: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    /// 开始时间 (格式：2023-01-01 00:00:00)
    pub start_ts: Option<String>,
    /// 结束时间 (格式：2023-01-01 23:59:59)
    pub end_ts: Option<String>,
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

    /// 检查是否提供了需要预扫描的过滤器 (Transaction-level)
    #[must_use]
    pub fn has_transaction_filters(&self) -> bool {
        if !self.enable {
            return false;
        }
        self.indicators.has_filters() || self.sql.has_filters()
    }

    /// 检查记录是否应该被保留
    /// 逻辑：(满足元数据过滤) OR (属于被选中的事务)
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
        if !self.enable {
            return true;
        }

        // 如果开启了过滤器功能，但没有任何具体的过滤项配置，保留所有记录。
        if !self.meta.has_filters() && !self.indicators.has_filters() && !self.sql.has_filters() {
            return true;
        }

        // Meta 过滤 (Record-level)
        self.meta
            .should_keep(ts, trxid, ip, sess, thrd, user, stmt, app, tag)
    }

    /// 合并预扫描发现的事务 ID 到 `MetaFilters` 中，以便在正式扫描时直接通过 trxid 匹配保留整笔事务
    pub fn merge_found_trxids(&mut self, trxids: Vec<String>) {
        if !self.enable || trxids.is_empty() {
            return;
        }
        let current = self.meta.trxids.take().unwrap_or_default();
        let mut set: HashSet<_> = current.into_iter().collect();
        for id in trxids {
            set.insert(id);
        }
        self.meta.trxids = Some(set.into_iter().collect());
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
            || self.start_ts.is_some()
            || self.end_ts.is_some()
    }

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
        // 时间范围过滤
        if let Some(start) = &self.start_ts {
            if ts < start.as_str() {
                return false;
            }
        }
        if let Some(end) = &self.end_ts {
            if ts > end.as_str() {
                return false;
            }
        }

        // 如果只有时间过滤器且已经通过，且没有其他过滤器，则保留
        let has_other_filters = self.trxids.as_ref().is_some_and(|v| !v.is_empty())
            || self.client_ips.as_ref().is_some_and(|v| !v.is_empty())
            || self.sess_ids.as_ref().is_some_and(|v| !v.is_empty())
            || self.thrd_ids.as_ref().is_some_and(|v| !v.is_empty())
            || self.usernames.as_ref().is_some_and(|v| !v.is_empty())
            || self.statements.as_ref().is_some_and(|v| !v.is_empty())
            || self.appnames.as_ref().is_some_and(|v| !v.is_empty())
            || self.tags.as_ref().is_some_and(|v| !v.is_empty());

        if !has_other_filters {
            return true;
        }

        // OR 逻辑：命中任何一个已定义的列表即保留
        Self::match_list(self.trxids.as_ref(), trxid, true)
            || Self::match_list(self.client_ips.as_ref(), ip, false)
            || Self::match_list(self.sess_ids.as_ref(), sess, false)
            || Self::match_list(self.thrd_ids.as_ref(), thrd, false)
            || Self::match_list(self.usernames.as_ref(), user, false)
            || Self::match_list(self.statements.as_ref(), stmt, false)
            || Self::match_list(self.appnames.as_ref(), app, false)
            || tag.is_some_and(|t| Self::match_list(self.tags.as_ref(), t, false))
    }

    fn match_list(list: Option<&Vec<String>>, val: &str, exact: bool) -> bool {
        if let Some(items) = list {
            if items.is_empty() {
                return false;
            }
            if exact {
                items.iter().any(|i| i == val)
            } else {
                items.iter().any(|i| val.contains(i))
            }
        } else {
            false
        }
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

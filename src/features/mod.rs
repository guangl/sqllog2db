pub mod filters;
pub use filters::FiltersFeature;

pub mod replace_parameters;
pub use replace_parameters::compute_normalized;

pub mod sql_fingerprint;
pub use sql_fingerprint::fingerprint;

use dm_database_parser_sqllog::{MetaParts, Sqllog};
use serde::Deserialize;

/// `[features.replace_parameters]` 配置段
#[derive(Debug, Deserialize, Clone)]
pub struct ReplaceParametersConfig {
    /// 是否在导出结果中写入 `normalized_sql` 列（默认 true）
    #[serde(default = "default_true")]
    pub enable: bool,
    /// 显式声明 SQL 中使用的占位符列表，例如 `["?"]` 或 `[":1"]`。
    /// - 只含 `"?"` → 仅匹配 `?` 顺序占位符
    /// - 含任意 `:N` 形式（如 `":1"`）→ 仅匹配 `:N` 序号占位符
    /// - 空数组（默认）→ 自动检测
    #[serde(default)]
    pub placeholders: Vec<String>,
}

impl Default for ReplaceParametersConfig {
    fn default() -> Self {
        Self {
            enable: true,
            placeholders: Vec::new(),
        }
    }
}

impl ReplaceParametersConfig {
    /// 将 `placeholders` 列表转换为 `compute_normalized` 所需的 `placeholder_override`：
    /// - `None`        → 自动检测
    /// - `Some(false)` → 强制 `?` 风格
    /// - `Some(true)`  → 强制 `:N` 风格
    #[must_use]
    pub fn placeholder_override(&self) -> Option<bool> {
        let has_question = self.placeholders.iter().any(|p| p == "?");
        let has_colon = self.placeholders.iter().any(|p| {
            p.starts_with(':') && p[1..].chars().next().is_some_and(|c| c.is_ascii_digit())
        });
        match (has_question, has_colon) {
            (true, false) => Some(false),
            (false, true) => Some(true),
            _ => None,
        }
    }
}

fn default_true() -> bool {
    true
}

/// 功能开关配置
#[derive(Debug, Deserialize, Clone, Default)]
pub struct FeaturesConfig {
    pub filters: Option<FiltersFeature>,
    pub replace_parameters: Option<ReplaceParametersConfig>,
}

/// 记录处理器接口：实现此接口即可加入处理管线
/// 返回 true 表示保留该记录，false 表示丢弃
pub trait LogProcessor: Send + Sync + std::fmt::Debug {
    fn process(&self, record: &Sqllog) -> bool;

    /// 使用调用方已预解析的 `MetaParts` 运行过滤逻辑，
    /// 消除 `parse_meta()` 的重复调用。
    /// 默认实现退化为 `process()`（向后兼容）。
    fn process_with_meta(&self, record: &Sqllog, _meta: &MetaParts<'_>) -> bool {
        self.process(record)
    }
}

/// 处理管线：按顺序执行处理器，任一返回 false 则丢弃记录
#[derive(Debug, Default)]
pub struct Pipeline {
    processors: Vec<Box<dyn LogProcessor>>,
}

impl Pipeline {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加处理器到管线末尾
    pub fn add(&mut self, processor: Box<dyn LogProcessor>) {
        self.processors.push(processor);
    }

    /// 管线是否为空（空管线可以走零开销快速路径）
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.processors.is_empty()
    }

    /// 顺序执行所有处理器（不共享预解析数据的兼容路径）
    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub fn run(&self, record: &Sqllog) -> bool {
        self.processors.iter().all(|p| p.process(record))
    }

    /// 使用已预解析的 `MetaParts` 顺序执行所有处理器，
    /// 避免各处理器内部重复调用 `parse_meta()`。
    #[inline]
    #[must_use]
    pub fn run_with_meta(&self, record: &Sqllog, meta: &MetaParts<'_>) -> bool {
        self.processors
            .iter()
            .all(|p| p.process_with_meta(record, meta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pipeline ───────────────────────────────────────────────
    #[test]
    fn test_pipeline_empty() {
        let p = Pipeline::new();
        assert!(p.is_empty());
    }

    #[test]
    fn test_pipeline_add() {
        let mut p = Pipeline::new();

        #[derive(Debug)]
        struct AlwaysPass;
        impl LogProcessor for AlwaysPass {
            fn process(&self, _: &dm_database_parser_sqllog::Sqllog) -> bool {
                true
            }
        }

        p.add(Box::new(AlwaysPass));
        assert!(!p.is_empty());
    }

    // ── ReplaceParametersConfig ────────────────────────────────
    #[test]
    fn test_placeholder_override_question() {
        let cfg = ReplaceParametersConfig {
            enable: true,
            placeholders: vec!["?".into()],
        };
        assert_eq!(cfg.placeholder_override(), Some(false));
    }

    #[test]
    fn test_placeholder_override_colon() {
        let cfg = ReplaceParametersConfig {
            enable: true,
            placeholders: vec![":1".into()],
        };
        assert_eq!(cfg.placeholder_override(), Some(true));
    }

    #[test]
    fn test_placeholder_override_auto() {
        let cfg = ReplaceParametersConfig {
            enable: true,
            placeholders: vec![],
        };
        assert_eq!(cfg.placeholder_override(), None);
    }

    #[test]
    fn test_placeholder_override_both_is_auto() {
        let cfg = ReplaceParametersConfig {
            enable: true,
            placeholders: vec!["?".into(), ":1".into()],
        };
        // Both → ambiguous → None
        assert_eq!(cfg.placeholder_override(), None);
    }

    // ── FeaturesConfig ─────────────────────────────────────────
    #[test]
    fn test_features_config_default() {
        let cfg = FeaturesConfig::default();
        assert!(cfg.filters.is_none());
        assert!(cfg.replace_parameters.is_none());
    }
}

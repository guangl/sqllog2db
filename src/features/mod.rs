pub mod filters;
pub use filters::FiltersFeature;

pub mod replace_parameters;
pub use replace_parameters::compute_normalized;

pub mod sql_fingerprint;
pub use sql_fingerprint::fingerprint;

use dm_database_parser_sqllog::{MetaParts, Sqllog};
use serde::Deserialize;

/// 导出字段名列表（顺序与 CSV/SQLite 列顺序一致，共 15 个字段）
pub const FIELD_NAMES: &[&str] = &[
    "ts",             // 0
    "ep",             // 1
    "sess_id",        // 2
    "thrd_id",        // 3
    "username",       // 4
    "trx_id",         // 5
    "statement",      // 6
    "appname",        // 7
    "client_ip",      // 8
    "tag",            // 9
    "sql",            // 10
    "exec_time_ms",   // 11
    "row_count",      // 12
    "exec_id",        // 13
    "normalized_sql", // 14
];

/// 字段投影掩码：u16 位图，bit i=1 表示导出第 i 个字段（共 15 个）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldMask(pub u16);

impl FieldMask {
    /// 全部 15 个字段都导出（默认值）
    pub const ALL: Self = Self(0x7FFF);

    /// 从字段名列表构建掩码，未知字段名返回错误消息
    pub fn from_names(names: &[String]) -> std::result::Result<Self, String> {
        let mut mask = 0u16;
        for name in names {
            match FIELD_NAMES.iter().position(|&n| n == name.as_str()) {
                Some(idx) => mask |= 1u16 << idx,
                None => return Err(format!("unknown field: '{name}'")),
            }
        }
        Ok(Self(mask))
    }

    /// 第 `idx` 个字段是否启用
    #[inline]
    #[must_use]
    pub fn is_active(self, idx: usize) -> bool {
        idx < 15 && (self.0 >> idx) & 1 == 1
    }

    /// `normalized_sql` 字段（索引 14）是否启用
    #[inline]
    #[must_use]
    pub fn includes_normalized_sql(self) -> bool {
        self.is_active(14)
    }
}

impl Default for FieldMask {
    fn default() -> Self {
        Self::ALL
    }
}

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
    /// 字段投影：仅导出指定字段，默认为全部 15 个字段
    pub fields: Option<Vec<String>>,
}

impl FeaturesConfig {
    /// 计算字段投影掩码。字段名在 `validate()` 阶段已验证，无效名称静默退化为全量掩码。
    #[must_use]
    pub fn field_mask(&self) -> FieldMask {
        match &self.fields {
            None => FieldMask::ALL,
            Some(names) => FieldMask::from_names(names).unwrap_or(FieldMask::ALL),
        }
    }
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

    #[test]
    fn test_replace_parameters_config_default() {
        let cfg = ReplaceParametersConfig::default();
        assert!(cfg.enable);
        assert!(cfg.placeholders.is_empty());
    }

    #[test]
    fn test_default_true_via_serde() {
        // TOML without `enable` field → serde calls default_true() → true
        let cfg: ReplaceParametersConfig = toml::from_str("").unwrap();
        assert!(cfg.enable);
    }

    #[test]
    fn test_process_with_meta_default_delegates_to_process() {
        use dm_database_parser_sqllog::LogParser;

        #[derive(Debug)]
        struct AlwaysPass;
        impl LogProcessor for AlwaysPass {
            fn process(&self, _: &dm_database_parser_sqllog::Sqllog) -> bool {
                true
            }
            // No process_with_meta override → uses default which calls process()
        }

        #[derive(Debug)]
        struct AlwaysFail;
        impl LogProcessor for AlwaysFail {
            fn process(&self, _: &dm_database_parser_sqllog::Sqllog) -> bool {
                false
            }
        }

        let dir = tempfile::TempDir::new().unwrap();
        let log = dir.path().join("t.log");
        std::fs::write(&log, "2025-01-15 10:30:28.001 (EP[0] sess:0x0001 user:U trxid:1 stmt:0x1 appname:A ip:10.0.0.1) [SEL] SELECT 1. EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n").unwrap();
        let parser = LogParser::from_path(log.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().flatten().collect();
        assert!(!records.is_empty());

        let record = &records[0];
        let meta = record.parse_meta();

        let mut p = Pipeline::new();
        p.add(Box::new(AlwaysPass));
        assert!(p.run_with_meta(record, &meta));

        let mut p2 = Pipeline::new();
        p2.add(Box::new(AlwaysFail));
        assert!(!p2.run_with_meta(record, &meta));
    }
}

use crate::error::{ConfigError, Error, Result};

pub const LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];
pub use crate::features::FeaturesConfig;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub sqllog: SqllogConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub features: FeaturesConfig,
    #[serde(default)]
    pub exporter: ExporterConfig,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|_| Error::Config(ConfigError::NotFound(path.to_path_buf())))?;
        toml::from_str(&content).map_err(|e| {
            Error::Config(ConfigError::ParseFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })
    }

    pub fn validate(&self) -> Result<()> {
        self.logging.validate()?;
        self.exporter.validate()?;
        self.sqllog.validate()?;
        FeaturesConfig::validate();
        Ok(())
    }

    /// 将 `--set key=value` 覆盖应用到 config。
    /// 支持点路径，例如 `sqllog.path`、`exporter.csv.file`。
    pub fn apply_overrides(&mut self, overrides: &[String]) -> Result<()> {
        for item in overrides {
            let (key, value) = item.split_once('=').ok_or_else(|| {
                Error::Config(ConfigError::InvalidValue {
                    field: item.clone(),
                    value: String::new(),
                    reason: "expected KEY=VALUE format".to_string(),
                })
            })?;
            self.apply_one(key, value)?;
        }
        Ok(())
    }

    fn apply_one(&mut self, key: &str, value: &str) -> Result<()> {
        let unknown = || {
            Error::Config(ConfigError::InvalidValue {
                field: key.to_string(),
                value: value.to_string(),
                reason: format!("unknown config key '{key}'"),
            })
        };
        let parse_bool = |v: &str| -> Result<bool> {
            match v {
                "true" | "1" | "yes" => Ok(true),
                "false" | "0" | "no" => Ok(false),
                _ => Err(Error::Config(ConfigError::InvalidValue {
                    field: key.to_string(),
                    value: v.to_string(),
                    reason: "expected true/false".to_string(),
                })),
            }
        };

        match key {
            "sqllog.path" | "sqllog.directory" => self.sqllog.path = value.to_string(),
            "logging.level" => self.logging.level = value.to_string(),
            "logging.file" => self.logging.file = value.to_string(),
            "logging.retention_days" => {
                self.logging.retention_days = value.parse().map_err(|_| {
                    Error::Config(ConfigError::InvalidValue {
                        field: key.to_string(),
                        value: value.to_string(),
                        reason: "expected a positive integer".to_string(),
                    })
                })?;
            }

            "exporter.csv.file" => {
                self.exporter.csv.get_or_insert_with(Default::default).file = value.to_string();
            }
            "exporter.csv.overwrite" => {
                self.exporter
                    .csv
                    .get_or_insert_with(Default::default)
                    .overwrite = parse_bool(value)?;
            }
            "exporter.csv.append" => {
                self.exporter
                    .csv
                    .get_or_insert_with(Default::default)
                    .append = parse_bool(value)?;
            }

            "exporter.sqlite.database_url" => {
                self.exporter
                    .sqlite
                    .get_or_insert_with(Default::default)
                    .database_url = value.to_string();
            }
            "exporter.sqlite.table_name" => {
                self.exporter
                    .sqlite
                    .get_or_insert_with(Default::default)
                    .table_name = value.to_string();
            }
            "exporter.sqlite.overwrite" => {
                self.exporter
                    .sqlite
                    .get_or_insert_with(Default::default)
                    .overwrite = parse_bool(value)?;
            }
            "exporter.sqlite.append" => {
                self.exporter
                    .sqlite
                    .get_or_insert_with(Default::default)
                    .append = parse_bool(value)?;
            }

            _ => return Err(unknown()),
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SqllogConfig {
    /// 日志文件路径：目录、单文件或 glob 模式（e.g. `sqllogs/*.log`）
    /// 旧配置中的 `directory` 键仍被接受。
    #[serde(alias = "directory")]
    pub path: String,
}

impl Default for SqllogConfig {
    fn default() -> Self {
        Self {
            path: "sqllogs".to_string(),
        }
    }
}

impl SqllogConfig {
    pub fn validate(&self) -> Result<()> {
        if self.path.trim().is_empty() {
            return Err(Error::Config(ConfigError::InvalidValue {
                field: "sqllog.path".to_string(),
                value: self.path.clone(),
                reason: "Input path cannot be empty".to_string(),
            }));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_logging_file")]
    pub file: String,
    #[serde(default = "default_logging_level")]
    pub level: String,
    #[serde(default = "default_retention_days")]
    pub retention_days: usize,
}

fn default_logging_file() -> String {
    "logs/sqllog2db.log".to_string()
}
fn default_logging_level() -> String {
    "info".to_string()
}
fn default_retention_days() -> usize {
    7
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            file: "logs/sqllog2db.log".to_string(),
            level: "info".to_string(),
            retention_days: 7,
        }
    }
}

impl LoggingConfig {
    pub fn validate(&self) -> Result<()> {
        if !LOG_LEVELS
            .iter()
            .any(|&l| l.eq_ignore_ascii_case(&self.level))
        {
            return Err(Error::Config(ConfigError::InvalidLogLevel {
                level: self.level.clone(),
                valid_levels: LOG_LEVELS.iter().map(|s| (*s).to_string()).collect(),
            }));
        }
        if self.retention_days == 0 || self.retention_days > 365 {
            return Err(Error::Config(ConfigError::InvalidValue {
                field: "logging.retention_days".to_string(),
                value: self.retention_days.to_string(),
                reason: "Retention days must be between 1 and 365".to_string(),
            }));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExporterConfig {
    pub csv: Option<CsvExporter>,
    pub sqlite: Option<SqliteExporter>,
}

impl ExporterConfig {
    fn has_any(&self) -> bool {
        self.csv.is_some() || self.sqlite.is_some()
    }

    pub fn validate(&self) -> Result<()> {
        if !self.has_any() {
            return Err(Error::Config(ConfigError::NoExporters));
        }
        Ok(())
    }
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            csv: Some(CsvExporter::default()),
            sqlite: None,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct CsvExporter {
    pub file: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    #[serde(default)]
    pub append: bool,
}

impl Default for CsvExporter {
    fn default() -> Self {
        Self {
            file: "outputs/sqllog.csv".to_string(),
            overwrite: true,
            append: false,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SqliteExporter {
    pub database_url: String,
    #[serde(default = "default_table_name")]
    pub table_name: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    #[serde(default)]
    pub append: bool,
}

fn default_table_name() -> String {
    "sqllog_records".to_string()
}

impl Default for SqliteExporter {
    fn default() -> Self {
        Self {
            database_url: "export/sqllog2db.db".to_string(),
            table_name: "sqllog_records".to_string(),
            overwrite: true,
            append: false,
        }
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> Config {
        Config::default()
    }

    // ── validate ───────────────────────────────────────────────
    #[test]
    fn test_validate_default_config_passes() {
        assert!(default_config().validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_log_level() {
        let mut cfg = default_config();
        cfg.logging.level = "invalid".into();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_retention_days_zero() {
        let mut cfg = default_config();
        cfg.logging.retention_days = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_retention_days_over_365() {
        let mut cfg = default_config();
        cfg.logging.retention_days = 366;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_empty_sqllog_directory() {
        let mut cfg = default_config();
        cfg.sqllog.path = "  ".into();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_no_exporters() {
        let mut cfg = default_config();
        cfg.exporter.csv = None;
        assert!(cfg.validate().is_err());
    }

    // ── apply_overrides ────────────────────────────────────────
    #[test]
    fn test_apply_overrides_sqllog_path() {
        let mut cfg = default_config();
        cfg.apply_overrides(&["sqllog.path=/tmp/logs".into()])
            .unwrap();
        assert_eq!(cfg.sqllog.path, "/tmp/logs");
    }

    #[test]
    fn test_apply_overrides_sqllog_directory_alias() {
        let mut cfg = default_config();
        cfg.apply_overrides(&["sqllog.directory=/tmp/logs".into()])
            .unwrap();
        assert_eq!(cfg.sqllog.path, "/tmp/logs");
    }

    #[test]
    fn test_apply_overrides_logging_level() {
        let mut cfg = default_config();
        cfg.apply_overrides(&["logging.level=debug".into()])
            .unwrap();
        assert_eq!(cfg.logging.level, "debug");
    }

    #[test]
    fn test_apply_overrides_csv_file() {
        let mut cfg = default_config();
        cfg.apply_overrides(&["exporter.csv.file=/tmp/out.csv".into()])
            .unwrap();
        assert_eq!(cfg.exporter.csv.unwrap().file, "/tmp/out.csv");
    }

    #[test]
    fn test_apply_overrides_csv_overwrite_false() {
        let mut cfg = default_config();
        cfg.apply_overrides(&["exporter.csv.overwrite=false".into()])
            .unwrap();
        assert!(!cfg.exporter.csv.unwrap().overwrite);
    }

    #[test]
    fn test_apply_overrides_sqlite_database_url() {
        let mut cfg = default_config();
        cfg.apply_overrides(&["exporter.sqlite.database_url=/tmp/out.db".into()])
            .unwrap();
        assert_eq!(cfg.exporter.sqlite.unwrap().database_url, "/tmp/out.db");
    }

    #[test]
    fn test_apply_overrides_unknown_key_returns_error() {
        let mut cfg = default_config();
        assert!(cfg.apply_overrides(&["unknown.key=value".into()]).is_err());
    }

    #[test]
    fn test_apply_overrides_bad_format_returns_error() {
        let mut cfg = default_config();
        assert!(cfg.apply_overrides(&["nodeleimiter".into()]).is_err());
    }

    #[test]
    fn test_apply_overrides_invalid_bool() {
        let mut cfg = default_config();
        assert!(
            cfg.apply_overrides(&["exporter.csv.overwrite=maybe".into()])
                .is_err()
        );
    }

    #[test]
    fn test_apply_overrides_retention_days_invalid() {
        let mut cfg = default_config();
        assert!(
            cfg.apply_overrides(&["logging.retention_days=abc".into()])
                .is_err()
        );
    }

    // ── ExporterConfig ─────────────────────────────────────────
    #[test]
    fn test_exporter_config_has_any_csv() {
        let cfg = ExporterConfig::default();
        assert!(cfg.csv.is_some());
    }

    #[test]
    fn test_exporter_config_default_no_sqlite() {
        let cfg = ExporterConfig::default();
        assert!(cfg.sqlite.is_none());
    }

    // ── from_file ──────────────────────────────────────────────
    #[test]
    fn test_from_file_not_found() {
        let result = Config::from_file("/nonexistent/path/config.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_file_valid_toml() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[sqllog]
directory = "sqllogs"
[exporter.csv]
file = "out.csv"
"#,
        )
        .unwrap();
        let cfg = Config::from_file(&path).unwrap();
        assert_eq!(cfg.sqllog.path, "sqllogs");
        assert_eq!(cfg.exporter.csv.unwrap().file, "out.csv");
    }
}

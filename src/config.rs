use crate::constants::LOG_LEVELS;
use crate::error::{ConfigError, Error, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    /// 新增：SQL 日志输入相关配置
    #[serde(default)]
    pub sqllog: SqllogConfig,

    pub error: ErrorConfig,
    pub logging: LoggingConfig,
    pub features: FeaturesConfig,
    pub exporter: ExporterConfig,
}

impl Config {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|_| Error::Config(ConfigError::NotFound(path.to_path_buf())))?;
        Self::from_str(&content, path.to_path_buf())
    }

    /// 从字符串解析配置
    pub fn from_str(content: &str, path: PathBuf) -> Result<Self> {
        let config: Config = toml::from_str(content).map_err(|e| {
            Error::Config(ConfigError::ParseFailed {
                path,
                reason: e.to_string(),
            })
        })?;

        // 验证配置
        config.validate()?;

        Ok(config)
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<()> {
        // 验证日志级别
        self.logging.validate()?;

        // 验证导出器配置
        self.exporter.validate()?;

        // 验证 sqllog 配置
        self.sqllog.validate()?;

        Ok(())
    }
}

/// SQL 日志输入配置
#[derive(Debug, Deserialize, Clone)]
pub struct SqllogConfig {
    /// SQL 日志输入目录（可包含多个日志文件）
    pub directory: String,
    /// 批量提交大小，0 表示全部解析完之后一次性写入，>0 表示每 N 条记录批量提交一次
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

impl Default for SqllogConfig {
    fn default() -> Self {
        Self {
            directory: "sqllog".to_string(),
            batch_size: 10000, // 默认使用 10000 批量大小以获得最佳性能
        }
    }
}

impl SqllogConfig {
    /// 获取 SQL 日志输入目录
    pub fn directory(&self) -> &str {
        &self.directory
    }

    /// 获取批量提交大小
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        if self.directory.trim().is_empty() {
            return Err(Error::Config(ConfigError::InvalidValue {
                field: "sqllog.directory".to_string(),
                value: self.directory.clone(),
                reason: "输入目录不能为空".to_string(),
            }));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ErrorConfig {
    /// 错误日志输出文件路径
    pub file: String,
}

impl ErrorConfig {
    /// 获取错误日志输出文件路径
    pub fn file(&self) -> &str {
        &self.file
    }
}

impl Default for ErrorConfig {
    fn default() -> Self {
        Self {
            file: "errors.json".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    /// 应用日志输出文件路径
    pub file: String,
    pub level: String,
    #[serde(default = "default_retention_days")]
    pub retention_days: usize,
}

fn default_retention_days() -> usize {
    7
}

impl LoggingConfig {
    /// 获取日志输出文件路径
    pub fn file(&self) -> &str {
        &self.file
    }

    /// 获取日志级别
    pub fn level(&self) -> &str {
        &self.level
    }

    /// 获取日志保留天数
    pub fn retention_days(&self) -> usize {
        self.retention_days
    }

    /// 验证日志级别是否有效
    pub fn validate(&self) -> Result<()> {
        if !LOG_LEVELS
            .iter()
            .any(|&l| l.eq_ignore_ascii_case(self.level.as_str()))
        {
            return Err(Error::Config(ConfigError::InvalidLogLevel {
                level: self.level.clone(),
                valid_levels: LOG_LEVELS.iter().map(|s| s.to_string()).collect(),
            }));
        }

        // 验证保留天数（1-365天）
        if self.retention_days == 0 || self.retention_days > 365 {
            return Err(Error::Config(ConfigError::InvalidValue {
                field: "logging.retention_days".to_string(),
                value: self.retention_days.to_string(),
                reason: "保留天数必须在 1-365 之间".to_string(),
            }));
        }

        Ok(())
    }
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

#[derive(Debug, Deserialize)]
pub struct FeaturesConfig {
    pub replace_sql_parameters: bool,
    pub scatter: bool,
}

impl FeaturesConfig {
    /// 是否启用 SQL 参数替换
    pub fn should_replace_sql_parameters(&self) -> bool {
        self.replace_sql_parameters
    }

    /// 是否启用散列功能
    pub fn should_scatter(&self) -> bool {
        self.scatter
    }
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            replace_sql_parameters: false,
            scatter: false,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ExporterConfig {
    pub database: Option<DatabaseExporter>,
    pub csv: Option<CsvExporter>,
}

impl ExporterConfig {
    /// 获取 CSV 导出器
    pub fn csv(&self) -> Option<&CsvExporter> {
        self.csv.as_ref()
    }

    /// 检查是否有任何导出器配置
    pub fn has_exporters(&self) -> bool {
        self.database.is_some() || self.csv.is_some()
    }

    /// 统计配置的导出器总数
    pub fn total_exporters(&self) -> usize {
        let mut count = 0;
        if self.database.is_some() {
            count += 1;
        }
        if self.csv.is_some() {
            count += 1;
        }
        count
    }

    /// 验证导出器配置（只支持单个导出器）
    pub fn validate(&self) -> Result<()> {
        if !self.has_exporters() {
            return Err(Error::Config(ConfigError::NoExporters));
        }

        let total = self.total_exporters();
        if total > 1 {
            eprintln!("警告: 配置了 {} 个导出器，但只支持单个导出器。", total);
            eprintln!("将按优先级使用第一个导出器：CSV > Database");
        }

        Ok(())
    }
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            database: None,
            csv: Some(CsvExporter::default()),
        }
    }
}

/// 支持的数据库类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    /// SQLite 数据库
    SQLite,
}

impl DatabaseType {
    /// 获取数据库类型的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseType::SQLite => "sqlite",
        }
    }
}

impl Default for DatabaseType {
    fn default() -> Self {
        DatabaseType::SQLite
    }
}

#[derive(Debug, Deserialize)]
pub struct DatabaseExporter {
    /// 数据库类型
    pub database_type: DatabaseType,

    // === 文件型数据库字段 (SQLite) ===
    /// 数据库输出文件路径
    #[serde(alias = "path")]
    pub file: String,

    // === 通用字段 ===
    /// 是否覆盖已存在的数据
    pub overwrite: bool,
    /// 目标表名
    pub table_name: String,
}

fn default_batch_size() -> usize {
    10000
}

impl DatabaseExporter {}

#[derive(Debug, Deserialize)]
pub struct CsvExporter {
    /// CSV 输出文件路径
    #[serde(alias = "path")]
    pub file: String,
    pub overwrite: bool,
}

impl CsvExporter {}

impl Default for CsvExporter {
    fn default() -> Self {
        Self {
            file: "export/sqllog2db.csv".to_string(),
            overwrite: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sqllog: SqllogConfig::default(),
            error: ErrorConfig::default(),
            logging: LoggingConfig::default(),
            features: FeaturesConfig::default(),
            exporter: ExporterConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    // 辅助函数：创建临时配置文件
    fn create_temp_config(content: &str, filename: &str) -> PathBuf {
        let path = PathBuf::from(filename);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    // 辅助函数：清理临时文件
    fn cleanup_temp_file(path: &PathBuf) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_config_from_nonexistent_file() {
        let result = Config::from_file("nonexistent_config.toml");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("配置文件未找到"));
    }

    #[test]
    fn test_config_invalid_toml() {
        let content = "invalid toml content {[}]";
        let path = create_temp_config(content, "test_config_invalid.toml");

        let result = Config::from_file(&path);
        cleanup_temp_file(&path);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("配置文件解析失败"));
    }

    #[test]
    fn test_config_invalid_log_level() {
        let content = r#"
[sqllog]
directory = "sqllog"
batch_size = 10000

[error]
file = "errors.json"

[logging]
file = "logs/app.log"
level = "verbose"

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "output.csv"
overwrite = true
"#;
        let path = create_temp_config(content, "test_config_invalid_level.toml");

        let result = Config::from_file(&path);
        cleanup_temp_file(&path);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("无效的日志级别"));
        assert!(err.to_string().contains("verbose"));
    }

    #[test]
    fn test_config_no_exporters() {
        let content = r#"
[sqllog]
directory = "sqllog"
batch_size = 10000

[error]
file = "errors.json"

[logging]
file = "logs/app.log"
level = "info"

[features]
replace_sql_parameters = false
scatter = false

[exporter]
"#;
        let path = create_temp_config(content, "test_config_no_exporters.toml");

        let result = Config::from_file(&path);
        cleanup_temp_file(&path);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("至少需要配置一个导出器"));
    }

    #[test]
    fn test_logging_config_validate_valid_levels() {
        let valid_levels = vec!["trace", "debug", "info", "warn", "error"];

        for level in valid_levels {
            let logging = LoggingConfig {
                file: "logs/app.log".to_string(),
                level: level.to_string(),
                retention_days: 7,
            };
            assert!(
                logging.validate().is_ok(),
                "Level {} should be valid",
                level
            );
        }
    }

    #[test]
    fn test_logging_config_validate_invalid_level() {
        let logging = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "invalid".to_string(),
            retention_days: 7,
        };

        let result = logging.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("无效的日志级别"));
    }

    #[test]
    fn test_logging_config_methods() {
        let logging = LoggingConfig {
            file: "logs/test.log".to_string(),
            level: "debug".to_string(),
            retention_days: 14,
        };

        assert_eq!(logging.file(), "logs/test.log");
        assert_eq!(logging.level(), "debug");
        assert_eq!(logging.retention_days(), 14);
    }

    #[test]
    fn test_logging_config_validate_retention_days() {
        // 有效的保留天数
        let valid = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };
        assert!(valid.validate().is_ok());

        // 保留天数为 0（无效）
        let invalid_zero = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 0,
        };
        assert!(invalid_zero.validate().is_err());

        // 保留天数超过 365（无效）
        let invalid_high = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 366,
        };
        assert!(invalid_high.validate().is_err());
    }

    #[test]
    fn test_error_config_path() {
        let error_config = ErrorConfig {
            file: "errors/app.json".to_string(),
        };

        assert_eq!(error_config.file(), "errors/app.json");
    }

    #[test]
    fn test_features_config_methods() {
        let features = FeaturesConfig {
            replace_sql_parameters: true,
            scatter: false,
        };

        assert!(features.should_replace_sql_parameters());
        assert!(!features.should_scatter());
    }

    #[test]
    fn test_config_from_str() {
        let content = r#"
[sqllog]
directory = "sqllog"
batch_size = 10000

[error]
file = "errors.json"

[logging]
file = "logs/app.log"
level = "info"

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "output.csv"
overwrite = true
"#;

        let result = Config::from_str(content, PathBuf::from("test.toml"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_sqllog_defaults_when_missing() {
        // 缺省 [sqllog] 时，启用默认值
        let content = r#"
[error]
file = "errors.json"

[logging]
file = "logs/app.log"
level = "info"

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "output.csv"
overwrite = true
"#;

        let cfg = Config::from_str(content, PathBuf::from("test.toml")).unwrap();
        assert_eq!(cfg.sqllog.directory(), "sqllog");
        assert_eq!(cfg.sqllog.batch_size(), 10000);
    }

    #[test]
    fn test_sqllog_from_file_values() {
        let content = r#"
[sqllog]
directory = "sqllog_dir"
batch_size = 5000

[error]
file = "errors.json"

[logging]
file = "logs/app.log"
level = "info"

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "output.csv"
overwrite = true
"#;
        let path = create_temp_config(content, "test_sqllog_values.toml");
        let cfg = Config::from_file(&path).unwrap();
        cleanup_temp_file(&path);

        assert_eq!(cfg.sqllog.directory(), "sqllog_dir");
        assert_eq!(cfg.sqllog.batch_size(), 5000);
    }

    #[test]
    fn test_config_validate_all_log_levels() {
        for level in &["trace", "debug", "info", "warn", "error"] {
            let content = format!(
                r#"
[sqllog]
directory = "sqllog"
batch_size = 10000

[error]
file = "errors.json"

[logging]
file = "logs/app.log"
level = "{}"

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "output.csv"
overwrite = true
"#,
                level
            );

            let filename = format!("test_config_level_{}.toml", level);
            let path = create_temp_config(&content, &filename);

            let result = Config::from_file(&path);
            cleanup_temp_file(&path);

            assert!(result.is_ok(), "Log level '{}' should be valid", level);
        }
    }

    #[test]
    fn test_config_default_validates() {
        let config = Config::default();
        // 默认配置应该能通过验证
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_error_config_default() {
        let error = ErrorConfig::default();
        assert_eq!(error.file(), "errors.json");
    }

    #[test]
    fn test_logging_config_default() {
        let logging = LoggingConfig::default();
        assert_eq!(logging.file(), "logs/sqllog2db.log");
        assert_eq!(logging.level(), "info");
        assert!(logging.validate().is_ok());
    }

    #[test]
    fn test_features_config_default() {
        let features = FeaturesConfig::default();
        assert!(!features.should_replace_sql_parameters());
        assert!(!features.should_scatter());
    }

    #[test]
    fn test_sqllog_config_default() {
        let sqllog = SqllogConfig::default();
        assert_eq!(sqllog.directory(), "sqllog");
        assert_eq!(sqllog.batch_size(), 10000);
        assert!(sqllog.validate().is_ok());
    }

    #[test]
    fn test_sqllog_config_validate_empty_path() {
        let cfg = SqllogConfig {
            directory: "   ".to_string(),
            batch_size: 100,
        };
        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("输入目录不能为空"));
    }

    #[test]
    fn test_sqllog_config_batch_size_default() {
        let sqllog = SqllogConfig::default();
        assert_eq!(sqllog.batch_size(), 10000); // 默认值应该是 10000
    }

    #[test]
    fn test_sqllog_config_batch_size_custom() {
        let sqllog = SqllogConfig {
            directory: "sqllog".to_string(),
            batch_size: 1000,
        };
        assert_eq!(sqllog.batch_size(), 1000);
        assert!(sqllog.validate().is_ok());
    }

    #[test]
    fn test_sqllog_config_from_toml_with_batch_size() {
        let toml_str = r#"
            [sqllog]
            directory = "test_sqllog"
            batch_size = 500

            [error]
            file = "errors.log"

            [logging]
            file = "logs/app.log"
            level = "info"

            [features]
            replace_sql_parameters = false
            scatter = false

            [exporter.csv]
            path = "test.csv"
            overwrite = true
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.sqllog.directory(), "test_sqllog");
        assert_eq!(config.sqllog.batch_size(), 500);
    }

    #[test]
    fn test_sqllog_config_from_toml_without_batch_size() {
        // 测试当 TOML 中没有 batch_size 时，使用默认值
        let toml_str = r#"
            [sqllog]
            directory = "test_logs"

            [error]
            file = "errors.log"

            [logging]
            file = "logs/app.log"
            level = "info"

            [features]
            replace_sql_parameters = false
            scatter = false

            [exporter.csv]
            path = "test.csv"
            overwrite = true
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.sqllog.directory, "test_logs");
        assert_eq!(config.sqllog.batch_size, 10000); // 默认值
    }
}

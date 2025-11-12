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
    /// SQL 日志目录或文件路径
    pub path: String,
    /// 批量提交大小，0 表示全部解析完之后一次性写入，>0 表示每 N 条记录批量提交一次
    #[serde(default)]
    pub batch_size: usize,
}

impl Default for SqllogConfig {
    fn default() -> Self {
        Self {
            path: "sqllog".to_string(),
            batch_size: 10000, // 默认使用 10000 批量大小以获得最佳性能
        }
    }
}

impl SqllogConfig {
    /// 获取 SQL 日志路径
    pub fn path(&self) -> &str {
        &self.path
    }

    /// 获取批量提交大小
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        if self.path.trim().is_empty() {
            return Err(Error::Config(ConfigError::InvalidValue {
                field: "sqllog.path".to_string(),
                value: self.path.clone(),
                reason: "路径不能为空".to_string(),
            }));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ErrorConfig {
    pub path: String,
}

impl ErrorConfig {
    /// 获取错误日志文件路径
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Default for ErrorConfig {
    fn default() -> Self {
        Self {
            path: "errors.jsonl".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub path: String,
    pub level: String,
    #[serde(default = "default_retention_days")]
    pub retention_days: usize,
}

fn default_retention_days() -> usize {
    7
}

impl LoggingConfig {
    /// 获取日志文件路径
    pub fn path(&self) -> &str {
        &self.path
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
            path: "logs/sqllog2db.log".to_string(),
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
    pub jsonl: Option<JsonlExporter>,
}

impl ExporterConfig {
    /// 获取数据库导出器
    pub fn database(&self) -> Option<&DatabaseExporter> {
        self.database.as_ref()
    }

    /// 获取 CSV 导出器
    pub fn csv(&self) -> Option<&CsvExporter> {
        self.csv.as_ref()
    }

    /// 获取 JSONL 导出器
    pub fn jsonl(&self) -> Option<&JsonlExporter> {
        self.jsonl.as_ref()
    }

    /// 检查是否有任何导出器配置
    pub fn has_exporters(&self) -> bool {
        self.database.is_some() || self.csv.is_some() || self.jsonl.is_some()
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
        if self.jsonl.is_some() {
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
            eprintln!("将按优先级使用第一个导出器：CSV > JSONL > Database");
        }

        Ok(())
    }
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            database: None,
            csv: Some(CsvExporter::default()),
            jsonl: None,
        }
    }
}

/// 支持的数据库类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    /// DuckDB 数据库
    DuckDB,
    /// SQLite 数据库
    SQLite,
    /// PostgreSQL 数据库
    #[serde(rename = "postgres")]
    PostgreSQL,
    /// Oracle 数据库
    Oracle,
    /// 达梦数据库
    #[serde(rename = "dm")]
    DM,
}

impl DatabaseType {
    /// 获取数据库类型的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseType::DuckDB => "duckdb",
            DatabaseType::SQLite => "sqlite",
            DatabaseType::PostgreSQL => "postgres",
            DatabaseType::Oracle => "oracle",
            DatabaseType::DM => "dm",
        }
    }

    /// 获取默认端口号
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn default_port(&self) -> u16 {
        match self {
            DatabaseType::DuckDB => 0,
            DatabaseType::SQLite => 0,
            DatabaseType::PostgreSQL => 5432,
            DatabaseType::Oracle => 1521,
            DatabaseType::DM => 5236,
        }
    }

    /// 是否为文件型数据库(不需要网络连接)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_file_based(&self) -> bool {
        matches!(self, DatabaseType::DuckDB | DatabaseType::SQLite)
    }
}

impl Default for DatabaseType {
    fn default() -> Self {
        DatabaseType::DM
    }
}

#[derive(Debug, Deserialize)]
pub struct DatabaseExporter {
    /// 数据库类型
    pub database_type: DatabaseType,

    // === 网络数据库字段 (DM, PostgreSQL, Oracle 等) ===
    /// 主机地址 (文件型数据库可留空)
    #[serde(default)]
    pub host: String,
    /// 端口号 (文件型数据库可为 0)
    #[serde(default)]
    pub port: u16,
    /// 用户名 (文件型数据库可留空)
    #[serde(default)]
    pub username: String,
    /// 密码 (文件型数据库可留空)
    #[serde(default)]
    pub password: String,
    /// 数据库名/Schema (可选, PostgreSQL/DM 使用)
    #[serde(default)]
    pub database: Option<String>,

    // === 文件型数据库字段 (SQLite, DuckDB) ===
    /// 数据库文件路径 (仅文件型数据库使用)
    #[serde(default)]
    pub path: Option<String>,

    // === Oracle 特殊字段 ===
    /// Oracle Service Name (可选)
    #[serde(default)]
    pub service_name: Option<String>,
    /// Oracle SID (可选)
    #[serde(default)]
    pub sid: Option<String>,

    // === 通用字段 ===
    /// 是否覆盖已存在的数据
    pub overwrite: bool,
    /// 目标表名
    pub table_name: String,
    /// 批量导入大小 (可选，默认 1000)
    #[serde(default = "default_db_batch_size")]
    pub batch_size: usize,
}

fn default_db_batch_size() -> usize {
    1000
}

impl DatabaseExporter {
    /// 获取数据库连接地址 (网络数据库)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// 是否覆盖已存在的数据
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn should_overwrite(&self) -> bool {
        self.overwrite
    }

    /// 获取表名
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    /// 获取数据库类型
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn database_type(&self) -> DatabaseType {
        self.database_type
    }

    /// 获取用户名
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// 获取密码
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn password(&self) -> &str {
        &self.password
    }

    /// 获取数据库名
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn database(&self) -> Option<&str> {
        self.database.as_deref()
    }

    /// 获取文件路径 (文件型数据库)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// 获取 Oracle Service Name
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn service_name(&self) -> Option<&str> {
        self.service_name.as_deref()
    }

    /// 获取 Oracle SID
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn sid(&self) -> Option<&str> {
        self.sid.as_deref()
    }

    /// 是否为文件型数据库
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_file_based(&self) -> bool {
        self.database_type.is_file_based()
    }

    /// 构建连接字符串 (根据数据库类型)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn build_connection_string(&self) -> String {
        match self.database_type {
            DatabaseType::SQLite | DatabaseType::DuckDB => self.path.clone().unwrap_or_default(),
            DatabaseType::PostgreSQL => {
                format!(
                    "postgresql://{}:{}@{}:{}/{}",
                    self.username,
                    self.password,
                    self.host,
                    self.port,
                    self.database.as_deref().unwrap_or("postgres")
                )
            }
            DatabaseType::Oracle => {
                if let Some(service) = &self.service_name {
                    format!("{}:{}/{}/{}", self.host, self.port, service, self.username)
                } else if let Some(sid) = &self.sid {
                    format!("{}:{}/{}/{}", self.host, self.port, sid, self.username)
                } else {
                    format!("{}:{}/{}", self.host, self.port, self.username)
                }
            }
            DatabaseType::DM => {
                format!(
                    "dm://{}:{}@{}:{}/{}",
                    self.username,
                    self.password,
                    self.host,
                    self.port,
                    self.database.as_deref().unwrap_or("SYSDBA")
                )
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CsvExporter {
    pub path: String,
    pub overwrite: bool,
}

impl CsvExporter {
    /// 获取导出文件路径
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// 是否覆盖已存在的文件
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn should_overwrite(&self) -> bool {
        self.overwrite
    }
}

impl Default for CsvExporter {
    fn default() -> Self {
        Self {
            path: "export/sqllog2db.csv".to_string(),
            overwrite: true,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct JsonlExporter {
    pub path: String,
    pub overwrite: bool,
}

impl JsonlExporter {
    /// 获取导出文件路径
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// 是否覆盖已存在的文件
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn should_overwrite(&self) -> bool {
        self.overwrite
    }
}

impl Default for JsonlExporter {
    fn default() -> Self {
        Self {
            path: "export/sqllog2db.jsonl".to_string(),
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
    fn test_config_from_valid_file() {
        let content = r#"
[sqllog]
path = "sqllog"
batch_size = 10000

[error]
path = "errors.jsonl"

[logging]
path = "logs/app.log"
level = "info"

[features]
replace_sql_parameters = false
scatter = false

[exporter.database]
database_type = "postgres"
host = "localhost"
port = 5236
username = "admin"
password = "password"
overwrite = true
table_name = "test_table"
"#;
        let path = create_temp_config(content, "test_config_valid.toml");

        let result = Config::from_file(&path);
        cleanup_temp_file(&path);

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.error.path, "errors.jsonl");
        assert_eq!(config.logging.path, "logs/app.log");
        assert_eq!(config.logging.level, "info");
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
path = "sqllog"
batch_size = 10000

[error]
path = "errors.jsonl"

[logging]
path = "logs/app.log"
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
path = "sqllog"
batch_size = 10000

[error]
path = "errors.jsonl"

[logging]
path = "logs/app.log"
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
                path: "logs/app.log".to_string(),
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
            path: "logs/app.log".to_string(),
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
            path: "logs/test.log".to_string(),
            level: "debug".to_string(),
            retention_days: 14,
        };

        assert_eq!(logging.path(), "logs/test.log");
        assert_eq!(logging.level(), "debug");
        assert_eq!(logging.retention_days(), 14);
    }

    #[test]
    fn test_logging_config_validate_retention_days() {
        // 有效的保留天数
        let valid = LoggingConfig {
            path: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };
        assert!(valid.validate().is_ok());

        // 保留天数为 0（无效）
        let invalid_zero = LoggingConfig {
            path: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 0,
        };
        assert!(invalid_zero.validate().is_err());

        // 保留天数超过 365（无效）
        let invalid_high = LoggingConfig {
            path: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 366,
        };
        assert!(invalid_high.validate().is_err());
    }

    #[test]
    fn test_error_config_path() {
        let error_config = ErrorConfig {
            path: "errors/app.jsonl".to_string(),
        };

        assert_eq!(error_config.path(), "errors/app.jsonl");
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
    fn test_exporter_config_database() {
        let mut exporter = ExporterConfig {
            database: Some(DatabaseExporter {
                database_type: DatabaseType::DM,
                host: "localhost".to_string(),
                port: 5236,
                username: "SYSDBA".to_string(),
                password: "SYSDBA".to_string(),
                database: Some("TESTDB".to_string()),
                path: None,
                service_name: None,
                sid: None,
                overwrite: true,
                table_name: "sqllog".to_string(),
                batch_size: 1000,
            }),
            csv: None,
            jsonl: None,
        };

        assert!(exporter.database().is_some());
        assert_eq!(exporter.database().unwrap().host, "localhost".to_string());
        assert_eq!(exporter.database().unwrap().database_type, DatabaseType::DM);

        exporter.database = None;
        assert!(exporter.database().is_none());
    }

    #[test]
    fn test_exporter_config_csv() {
        let mut exporter = ExporterConfig {
            database: None,
            csv: Some(CsvExporter {
                path: "output.csv".to_string(),
                overwrite: true,
            }),
            jsonl: None,
        };

        assert!(exporter.csv().is_some());
        assert_eq!(exporter.csv().unwrap().path, "output.csv");

        exporter.csv = None;
        assert!(exporter.csv().is_none());
    }

    #[test]
    fn test_exporter_config_jsonl() {
        let mut exporter = ExporterConfig {
            database: None,
            csv: None,
            jsonl: Some(JsonlExporter {
                path: "output.jsonl".to_string(),
                overwrite: false,
            }),
        };

        assert!(exporter.jsonl().is_some());
        assert_eq!(exporter.jsonl().unwrap().path, "output.jsonl");

        exporter.jsonl = None;
        assert!(exporter.jsonl().is_none());
    }

    #[test]
    fn test_exporter_config_has_exporters() {
        let exporter_with_db = ExporterConfig {
            database: Some(DatabaseExporter {
                database_type: DatabaseType::PostgreSQL,
                host: "localhost".to_string(),
                port: 5236,
                username: "admin".to_string(),
                password: "pass".to_string(),
                database: Some("mydb".to_string()),
                path: None,
                service_name: None,
                sid: None,
                overwrite: true,
                table_name: "table1".to_string(),
                batch_size: 1000,
            }),
            csv: None,
            jsonl: None,
        };
        assert!(exporter_with_db.has_exporters());

        let exporter_with_csv = ExporterConfig {
            database: None,
            csv: Some(CsvExporter {
                path: "output.csv".to_string(),
                overwrite: true,
            }),
            jsonl: None,
        };
        assert!(exporter_with_csv.has_exporters());

        let exporter_empty = ExporterConfig {
            database: None,
            csv: None,
            jsonl: None,
        };
        assert!(!exporter_empty.has_exporters());
    }

    #[test]
    fn test_exporter_config_validate() {
        let valid_exporter = ExporterConfig {
            database: Some(DatabaseExporter {
                database_type: DatabaseType::Oracle,
                host: "localhost".to_string(),
                port: 5236,
                username: "admin".to_string(),
                password: "pass".to_string(),
                database: None,
                path: None,
                service_name: Some("ORCL".to_string()),
                sid: None,
                overwrite: true,
                table_name: "table1".to_string(),
                batch_size: 1000,
            }),
            csv: None,
            jsonl: None,
        };
        assert!(valid_exporter.validate().is_ok());

        let invalid_exporter = ExporterConfig {
            database: None,
            csv: None,
            jsonl: None,
        };
        assert!(invalid_exporter.validate().is_err());
    }

    #[test]
    fn test_database_exporter_methods() {
        let db = DatabaseExporter {
            database_type: DatabaseType::SQLite,
            host: String::new(),
            port: 0,
            username: String::new(),
            password: String::new(),
            database: None,
            path: Some("/path/to/db.sqlite".to_string()),
            service_name: None,
            sid: None,
            overwrite: true,
            table_name: "my_table".to_string(),
            batch_size: 1000,
        };

        assert_eq!(db.database_type, DatabaseType::SQLite);
        assert_eq!(db.database_type(), DatabaseType::SQLite);
        assert_eq!(db.table_name(), "my_table");
        assert_eq!(db.path(), Some("/path/to/db.sqlite"));
        assert!(db.is_file_based()); // SQLite 是文件型数据库
        assert!(db.should_overwrite());
    }

    #[test]
    fn test_database_type_properties() {
        // 测试 DM 数据库
        assert_eq!(DatabaseType::DM.as_str(), "dm");
        assert_eq!(DatabaseType::DM.default_port(), 5236);
        assert!(!DatabaseType::DM.is_file_based());

        // 测试 PostgreSQL
        assert_eq!(DatabaseType::PostgreSQL.as_str(), "postgres");
        assert_eq!(DatabaseType::PostgreSQL.default_port(), 5432);
        assert!(!DatabaseType::PostgreSQL.is_file_based());

        // 测试 Oracle
        assert_eq!(DatabaseType::Oracle.as_str(), "oracle");
        assert_eq!(DatabaseType::Oracle.default_port(), 1521);
        assert!(!DatabaseType::Oracle.is_file_based());

        // 测试文件型数据库
        assert_eq!(DatabaseType::SQLite.as_str(), "sqlite");
        assert_eq!(DatabaseType::SQLite.default_port(), 0);
        assert!(DatabaseType::SQLite.is_file_based());

        assert_eq!(DatabaseType::DuckDB.as_str(), "duckdb");
        assert_eq!(DatabaseType::DuckDB.default_port(), 0);
        assert!(DatabaseType::DuckDB.is_file_based());
    }

    #[test]
    fn test_database_type_default() {
        let default_type = DatabaseType::default();
        assert_eq!(default_type, DatabaseType::DM);
    }

    #[test]
    fn test_database_type_deserialization() {
        // 测试从配置文件反序列化
        let toml_str = r#"
database_type = "dm"
host = "localhost"
port = 5236
username = "admin"
password = "pass"
overwrite = true
table_name = "test"
"#;
        let db: DatabaseExporter = toml::from_str(toml_str).unwrap();
        assert_eq!(db.database_type, DatabaseType::DM);

        // 测试 postgres 别名
        let toml_str_pg = r#"
database_type = "postgres"
host = "localhost"
port = 5432
username = "admin"
password = "pass"
overwrite = true
table_name = "test"
"#;
        let db_pg: DatabaseExporter = toml::from_str(toml_str_pg).unwrap();
        assert_eq!(db_pg.database_type, DatabaseType::PostgreSQL);

        // 测试 sqlite
        let toml_str_sqlite = r#"
database_type = "sqlite"
host = ""
port = 0
username = ""
password = ""
overwrite = true
table_name = "test"
"#;
        let db_sqlite: DatabaseExporter = toml::from_str(toml_str_sqlite).unwrap();
        assert_eq!(db_sqlite.database_type, DatabaseType::SQLite);
        assert!(db_sqlite.is_file_based());
    }

    #[test]
    fn test_database_connection_string() {
        // 测试 PostgreSQL 连接字符串
        let pg_db = DatabaseExporter {
            database_type: DatabaseType::PostgreSQL,
            host: "pg.example.com".to_string(),
            port: 5432,
            username: "pguser".to_string(),
            password: "pgpass".to_string(),
            database: Some("mydb".to_string()),
            path: None,
            service_name: None,
            sid: None,
            overwrite: false,
            table_name: "logs".to_string(),
            batch_size: 1000,
        };
        assert_eq!(
            pg_db.build_connection_string(),
            "postgresql://pguser:pgpass@pg.example.com:5432/mydb"
        );
        assert_eq!(pg_db.database(), Some("mydb"));

        // 测试 SQLite 连接字符串
        let sqlite_db = DatabaseExporter {
            database_type: DatabaseType::SQLite,
            host: String::new(),
            port: 0,
            username: String::new(),
            password: String::new(),
            database: None,
            path: Some("/data/app.db".to_string()),
            service_name: None,
            sid: None,
            overwrite: true,
            table_name: "events".to_string(),
            batch_size: 1000,
        };
        assert_eq!(sqlite_db.build_connection_string(), "/data/app.db");
        assert!(sqlite_db.is_file_based());

        // 测试 Oracle 连接字符串 (使用 service_name)
        let oracle_db = DatabaseExporter {
            database_type: DatabaseType::Oracle,
            host: "oracle.example.com".to_string(),
            port: 1521,
            username: "system".to_string(),
            password: "oracle".to_string(),
            database: None,
            path: None,
            service_name: Some("ORCL".to_string()),
            sid: None,
            overwrite: false,
            table_name: "audit_logs".to_string(),
            batch_size: 1000,
        };
        assert_eq!(
            oracle_db.build_connection_string(),
            "oracle.example.com:1521/ORCL/system"
        );
        assert_eq!(oracle_db.service_name(), Some("ORCL"));

        // 测试 DM 连接字符串
        let dm_db = DatabaseExporter {
            database_type: DatabaseType::DM,
            host: "dm.example.com".to_string(),
            port: 5236,
            username: "SYSDBA".to_string(),
            password: "SYSDBA".to_string(),
            database: Some("TEST".to_string()),
            path: None,
            service_name: None,
            sid: None,
            overwrite: true,
            table_name: "sqllog".to_string(),
            batch_size: 1000,
        };
        assert_eq!(
            dm_db.build_connection_string(),
            "dm://SYSDBA:SYSDBA@dm.example.com:5236/TEST"
        );
        assert_eq!(dm_db.username(), "SYSDBA");
        assert_eq!(dm_db.password(), "SYSDBA");
    }

    #[test]
    fn test_csv_exporter_methods() {
        let csv = CsvExporter {
            path: "/tmp/output.csv".to_string(),
            overwrite: false,
        };

        assert_eq!(csv.path(), "/tmp/output.csv");
        assert!(!csv.should_overwrite());
    }

    #[test]
    fn test_jsonl_exporter_methods() {
        let jsonl = JsonlExporter {
            path: "/tmp/output.jsonl".to_string(),
            overwrite: true,
        };

        assert_eq!(jsonl.path(), "/tmp/output.jsonl");
        assert!(jsonl.should_overwrite());
    }

    #[test]
    fn test_config_with_single_csv_exporter() {
        let toml_str = r#"
[sqllog]
path = "sqllogs"
batch_size = 10000

[error]
path = "errors.jsonl"

[logging]
path = "logs/sqllog2db.log"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = true
scatter = true

[exporter.csv]
path = "output.csv"
overwrite = true
"#;

        let result = Config::from_str(toml_str, PathBuf::from("test_config.toml"));
        assert!(result.is_ok());
        let config = result.unwrap();

        assert!(config.exporter.csv().is_some());
        assert_eq!(config.exporter.csv().unwrap().path, "output.csv");
        assert!(config.exporter.database().is_none());
        assert!(config.exporter.jsonl().is_none());

        assert!(config.features.should_replace_sql_parameters());
        assert!(config.features.should_scatter());
    }

    #[test]
    fn test_config_from_str() {
        let content = r#"
[sqllog]
path = "sqllog"
batch_size = 10000

[error]
path = "errors.jsonl"

[logging]
path = "logs/app.log"
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
path = "errors.jsonl"

[logging]
path = "logs/app.log"
level = "info"

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "output.csv"
overwrite = true
"#;

        let cfg = Config::from_str(content, PathBuf::from("test.toml")).unwrap();
        assert_eq!(cfg.sqllog.path(), "sqllog");
        assert_eq!(cfg.sqllog.batch_size(), 10000);
    }

    #[test]
    fn test_sqllog_from_file_values() {
        let content = r#"
[sqllog]
path = "sqllog_dir"
batch_size = 5000

[error]
path = "errors.jsonl"

[logging]
path = "logs/app.log"
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

        assert_eq!(cfg.sqllog.path(), "sqllog_dir");
        assert_eq!(cfg.sqllog.batch_size(), 5000);
    }

    #[test]
    fn test_config_validate_all_log_levels() {
        for level in &["trace", "debug", "info", "warn", "error"] {
            let content = format!(
                r#"
[sqllog]
path = "sqllog"
batch_size = 10000

[error]
path = "errors.jsonl"

[logging]
path = "logs/app.log"
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
    fn test_config_default() {
        let config = Config::default();

        // 验证 sqllog 默认值
        assert_eq!(config.sqllog.path(), "sqllog");
        assert_eq!(config.sqllog.batch_size(), 10000);

        // 验证 error 默认值
        assert_eq!(config.error.path(), "errors.jsonl");

        // 验证 logging 默认值
        assert_eq!(config.logging.path(), "logs/sqllog2db.log");
        assert_eq!(config.logging.level(), "info");

        // 验证 features 默认值
        assert!(!config.features.should_replace_sql_parameters());
        assert!(!config.features.should_scatter());
        let config = Config::default();

        // 验证 exporter 默认值：应该只有 CSV，没有 JSONL 和 Database
        assert!(config.exporter.csv().is_some());
        assert!(config.exporter.jsonl().is_none());
        assert!(config.exporter.database().is_none());

        // 验证默认 CSV 导出器配置
        let csv = config.exporter.csv().unwrap();
        assert_eq!(csv.path(), "export/sqllog2db.csv");
        assert!(csv.should_overwrite());
    }

    #[test]
    fn test_config_default_validates() {
        let config = Config::default();
        // 默认配置应该能通过验证
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_exporter_config_default_is_csv() {
        let exporter = ExporterConfig::default();

        // 默认应该只有 CSV 导出器
        assert!(exporter.csv.is_some());
        assert!(exporter.jsonl.is_none());
        assert!(exporter.database.is_none());

        // 应该有导出器
        assert!(exporter.has_exporters());

        // 应该能通过验证
        assert!(exporter.validate().is_ok());
    }

    #[test]
    fn test_csv_exporter_default() {
        let csv = CsvExporter::default();
        assert_eq!(csv.path(), "export/sqllog2db.csv");
        assert!(csv.should_overwrite());
    }

    #[test]
    fn test_jsonl_exporter_default() {
        let jsonl = JsonlExporter::default();
        assert_eq!(jsonl.path(), "export/sqllog2db.jsonl");
        assert!(jsonl.should_overwrite());
    }

    #[test]
    fn test_error_config_default() {
        let error = ErrorConfig::default();
        assert_eq!(error.path(), "errors.jsonl");
    }

    #[test]
    fn test_logging_config_default() {
        let logging = LoggingConfig::default();
        assert_eq!(logging.path(), "logs/sqllog2db.log");
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
        assert_eq!(sqllog.path(), "sqllog");
        assert_eq!(sqllog.batch_size(), 10000);
        assert!(sqllog.validate().is_ok());
    }

    #[test]
    fn test_sqllog_config_validate_empty_path() {
        let cfg = SqllogConfig {
            path: "   ".to_string(),
            batch_size: 100,
        };
        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("路径不能为空"));
    }

    #[test]
    fn test_sqllog_config_batch_size_default() {
        let sqllog = SqllogConfig::default();
        assert_eq!(sqllog.batch_size(), 10000); // 默认值应该是 10000
    }

    #[test]
    fn test_sqllog_config_batch_size_custom() {
        let sqllog = SqllogConfig {
            path: "sqllog".to_string(),
            batch_size: 1000,
        };
        assert_eq!(sqllog.batch_size(), 1000);
        assert!(sqllog.validate().is_ok());
    }

    #[test]
    fn test_sqllog_config_from_toml_with_batch_size() {
        let toml_str = r#"
            [sqllog]
            path = "test_sqllog"
            batch_size = 500

            [error]
            path = "errors.log"

            [logging]
            path = "logs/app.log"
            level = "info"

            [features]
            replace_sql_parameters = false
            scatter = false

            [exporter]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.sqllog.path(), "test_sqllog");
        assert_eq!(config.sqllog.batch_size(), 500);
    }

    #[test]
    fn test_sqllog_config_from_toml_without_batch_size() {
        // 测试当 TOML 中没有 batch_size 时，使用默认值
        let toml_str = r#"
            [sqllog]
            path = "test_logs"
            thread_count = 4

            [error]
            path = "errors.log"

            [logging]
            path = "logs/app.log"
            level = "info"

            [features]
            replace_sql_parameters = false
            scatter = false

            [exporter]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.sqllog.path, "test_logs");
        assert_eq!(config.sqllog.batch_size, 0); // 默认值
    }
}

use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// 应用程序错误类型
#[derive(Debug, Error)]
pub enum Error {
    /// 配置相关错误
    #[error("配置错误: {0}")]
    Config(#[from] ConfigError),

    /// 文件操作错误
    #[error("文件错误: {0}")]
    File(#[from] FileError),

    /// 数据库操作错误
    #[error("数据库错误: {0}")]
    Database(#[from] DatabaseError),

    /// 解析错误
    #[error("解析错误: {0}")]
    Parse(#[from] ParseError),

    /// SQL 日志解析器错误
    #[error("SQL日志解析器错误: {0}")]
    Parser(#[from] ParserError),

    /// 导出错误
    #[error("导出错误: {0}")]
    Export(#[from] ExportError),

    /// IO 错误
    #[error("IO错误: {0}")]
    Io(#[from] io::Error),

    /// 其他错误
    #[error("{0}")]
    Other(String),
}

/// 配置错误
#[derive(Debug, Error)]
pub enum ConfigError {
    /// 配置文件未找到
    #[error("配置文件未找到: {0}")]
    NotFound(PathBuf),

    /// 配置文件解析失败
    #[error("配置文件解析失败 {path}: {reason}")]
    ParseFailed { path: PathBuf, reason: String },

    /// 无效的日志级别
    #[error("无效的日志级别 '{level}', 有效值为: {}", valid_levels.join(", "))]
    InvalidLogLevel {
        level: String,
        valid_levels: Vec<String>,
    },

    /// 无效的配置值
    #[error("无效的配置值 {field} = '{value}': {reason}")]
    InvalidValue {
        field: String,
        value: String,
        reason: String,
    },

    /// 缺少必需的配置
    #[error("缺少必需的配置项: {field}")]
    MissingRequired { field: String },

    /// 没有配置导出器
    #[error("至少需要配置一个导出器 (database/csv/jsonl)")]
    NoExporters,
}

/// 文件操作错误
#[derive(Debug, Error)]
pub enum FileError {
    /// 文件未找到
    #[error("文件未找到: {0}")]
    NotFound(PathBuf),

    /// 文件已存在且不允许覆盖
    #[error("文件已存在: {path} (设置 overwrite=true 以覆盖)")]
    AlreadyExists { path: PathBuf },

    /// 无法创建文件
    #[error("无法创建文件 {path}: {reason}")]
    CreateFailed { path: PathBuf, reason: String },

    /// 无法读取文件
    #[error("无法读取文件 {path}: {reason}")]
    ReadFailed { path: PathBuf, reason: String },

    /// 无法写入文件
    #[error("无法写入文件 {path}: {reason}")]
    WriteFailed { path: PathBuf, reason: String },

    /// 无法创建目录
    #[error("无法创建目录 {path}: {reason}")]
    CreateDirectoryFailed { path: PathBuf, reason: String },

    /// 文件权限不足
    #[error("权限不足: 无法{operation} {path}")]
    PermissionDenied { path: PathBuf, operation: String },
}

/// 数据库错误
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// 连接失败
    #[error("数据库连接失败 {host}:{port}: {reason}")]
    ConnectionFailed {
        host: String,
        port: u16,
        reason: String,
    },

    /// 认证失败
    #[error("数据库认证失败: 用户 '{username}'")]
    AuthenticationFailed { username: String },

    /// 查询执行失败
    #[error("查询执行失败: {query}\n原因: {reason}")]
    QueryFailed { query: String, reason: String },

    /// 表不存在
    #[error("表不存在: {table_name}")]
    TableNotFound { table_name: String },

    /// 表已存在
    #[error("表已存在: {table_name} (设置 overwrite=true 以删除重建)")]
    TableAlreadyExists { table_name: String },

    /// 插入数据失败
    #[error("插入数据失败到表 {table_name}: {reason}")]
    InsertFailed { table_name: String, reason: String },

    /// 事务失败
    #[error("事务执行失败: {reason}")]
    TransactionFailed { reason: String },
}

/// 解析错误
#[derive(Debug, Error)]
pub enum ParseError {
    /// SQL 日志解析失败
    #[error("SQL日志解析失败 (行 {line_number}): {reason}\n内容: {content}")]
    SqlLogParseFailed {
        line_number: usize,
        content: String,
        reason: String,
    },

    /// 无效的 SQL 语句
    #[error("无效的SQL语句: {sql}\n原因: {reason}")]
    InvalidSql { sql: String, reason: String },

    /// 无效的时间格式
    #[error("无效的时间戳 '{value}', 期望格式: {expected_format}")]
    InvalidTimestamp {
        value: String,
        expected_format: String,
    },

    /// 无效的数值
    #[error("无效的数值 '{value}', 期望类型: {expected_type}")]
    InvalidNumber {
        value: String,
        expected_type: String,
    },

    /// CSV 解析失败
    #[error("CSV解析失败 (行 {line_number}): {reason}")]
    CsvParseFailed { line_number: usize, reason: String },

    /// JSON 解析失败
    #[error("JSON解析失败: {reason}")]
    JsonParseFailed { reason: String },
}

/// SQL 日志解析器错误
#[derive(Debug, Error)]
pub enum ParserError {
    /// 路径不存在
    #[error("路径不存在: {path}")]
    PathNotFound { path: PathBuf },

    /// 无效的路径
    #[error("无效的路径 {path}: {reason}")]
    InvalidPath { path: PathBuf, reason: String },

    /// 读取目录失败
    #[error("读取目录失败 {path}: {reason}")]
    ReadDirFailed { path: PathBuf, reason: String },

    /// 解析失败
    #[error("SQL日志解析失败: {reason}")]
    ParseFailed { reason: String },
}

/// 导出错误
#[derive(Debug, Error)]
pub enum ExportError {
    /// CSV 导出失败
    #[error("CSV导出失败 {path}: {reason}")]
    CsvExportFailed { path: PathBuf, reason: String },

    /// JSONL 导出失败
    #[error("JSONL导出失败 {path}: {reason}")]
    JsonlExportFailed { path: PathBuf, reason: String },

    /// 数据库导出失败
    #[error("数据库导出失败到表 {table_name}: {reason}")]
    DatabaseExportFailed { table_name: String, reason: String },

    /// 序列化失败
    #[error("序列化失败 (类型: {data_type}): {reason}")]
    SerializationFailed { data_type: String, reason: String },
}

/// 应用程序 Result 类型别名
pub type Result<T> = std::result::Result<T, Error>;

// 辅助宏，用于快速创建错误
#[macro_export]
macro_rules! config_error {
    ($variant:ident { $($field:ident: $value:expr),+ $(,)? }) => {
        $crate::error::Error::Config($crate::error::ConfigError::$variant {
            $($field: $value),+
        })
    };
}

#[macro_export]
macro_rules! file_error {
    ($variant:ident { $($field:ident: $value:expr),+ $(,)? }) => {
        $crate::error::Error::File($crate::error::FileError::$variant {
            $($field: $value),+
        })
    };
}

#[macro_export]
macro_rules! database_error {
    ($variant:ident { $($field:ident: $value:expr),+ $(,)? }) => {
        $crate::error::Error::Database($crate::error::DatabaseError::$variant {
            $($field: $value),+
        })
    };
}

#[macro_export]
macro_rules! parse_error {
    ($variant:ident { $($field:ident: $value:expr),+ $(,)? }) => {
        $crate::error::Error::Parse($crate::error::ParseError::$variant {
            $($field: $value),+
        })
    };
}

#[macro_export]
macro_rules! export_error {
    ($variant:ident { $($field:ident: $value:expr),+ $(,)? }) => {
        $crate::error::Error::Export($crate::error::ExportError::$variant {
            $($field: $value),+
        })
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_config_error_not_found() {
        let err = ConfigError::NotFound(PathBuf::from("test.toml"));
        let msg = err.to_string();
        assert!(msg.contains("配置文件未找到"));
        assert!(msg.contains("test.toml"));
    }

    #[test]
    fn test_config_error_parse_failed() {
        let err = ConfigError::ParseFailed {
            path: PathBuf::from("config.toml"),
            reason: "无效的TOML格式".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("配置文件解析失败"));
        assert!(msg.contains("config.toml"));
        assert!(msg.contains("无效的TOML格式"));
    }

    #[test]
    fn test_config_error_invalid_log_level() {
        let err = ConfigError::InvalidLogLevel {
            level: "verbose".to_string(),
            valid_levels: vec!["trace".to_string(), "debug".to_string(), "info".to_string()],
        };
        let msg = err.to_string();
        assert!(msg.contains("无效的日志级别"));
        assert!(msg.contains("verbose"));
        assert!(msg.contains("trace"));
        assert!(msg.contains("debug"));
        assert!(msg.contains("info"));
    }

    #[test]
    fn test_config_error_no_exporters() {
        let err = ConfigError::NoExporters;
        let msg = err.to_string();
        assert!(msg.contains("至少需要配置一个导出器"));
        assert!(msg.contains("database"));
        assert!(msg.contains("csv"));
        assert!(msg.contains("jsonl"));
    }

    #[test]
    fn test_file_error_not_found() {
        let err = FileError::NotFound(PathBuf::from("data.txt"));
        let msg = err.to_string();
        assert!(msg.contains("文件未找到"));
        assert!(msg.contains("data.txt"));
    }

    #[test]
    fn test_file_error_already_exists() {
        let err = FileError::AlreadyExists {
            path: PathBuf::from("output.csv"),
        };
        let msg = err.to_string();
        assert!(msg.contains("文件已存在"));
        assert!(msg.contains("output.csv"));
        assert!(msg.contains("overwrite=true"));
    }

    #[test]
    fn test_file_error_permission_denied() {
        let err = FileError::PermissionDenied {
            path: PathBuf::from("/etc/config.toml"),
            operation: "写入".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("权限不足"));
        assert!(msg.contains("写入"));
        assert!(msg.contains("config.toml"));
    }

    #[test]
    fn test_database_error_connection_failed() {
        let err = DatabaseError::ConnectionFailed {
            host: "localhost".to_string(),
            port: 5236,
            reason: "连接超时".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("数据库连接失败"));
        assert!(msg.contains("localhost:5236"));
        assert!(msg.contains("连接超时"));
    }

    #[test]
    fn test_database_error_authentication_failed() {
        let err = DatabaseError::AuthenticationFailed {
            username: "admin".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("数据库认证失败"));
        assert!(msg.contains("admin"));
    }

    #[test]
    fn test_database_error_table_already_exists() {
        let err = DatabaseError::TableAlreadyExists {
            table_name: "sqllog".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("表已存在"));
        assert!(msg.contains("sqllog"));
        assert!(msg.contains("overwrite=true"));
    }

    #[test]
    fn test_parse_error_sql_log_parse_failed() {
        let err = ParseError::SqlLogParseFailed {
            line_number: 42,
            content: "INVALID SQL LINE".to_string(),
            reason: "无法识别的日志格式".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("SQL日志解析失败"));
        assert!(msg.contains("行 42"));
        assert!(msg.contains("INVALID SQL LINE"));
        assert!(msg.contains("无法识别的日志格式"));
    }

    #[test]
    fn test_parse_error_invalid_timestamp() {
        let err = ParseError::InvalidTimestamp {
            value: "2023-13-45".to_string(),
            expected_format: "YYYY-MM-DD HH:MM:SS".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("无效的时间戳"));
        assert!(msg.contains("2023-13-45"));
        assert!(msg.contains("YYYY-MM-DD HH:MM:SS"));
    }

    #[test]
    fn test_export_error_csv_export_failed() {
        let err = ExportError::CsvExportFailed {
            path: PathBuf::from("output.csv"),
            reason: "磁盘空间不足".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("CSV导出失败"));
        assert!(msg.contains("output.csv"));
        assert!(msg.contains("磁盘空间不足"));
    }

    #[test]
    fn test_top_level_error_from_config() {
        let config_err = ConfigError::NoExporters;
        let err: Error = config_err.into();
        let msg = err.to_string();
        assert!(msg.contains("配置错误"));
        assert!(msg.contains("至少需要配置一个导出器"));
    }

    #[test]
    fn test_top_level_error_from_database() {
        let db_err = DatabaseError::ConnectionFailed {
            host: "localhost".to_string(),
            port: 5236,
            reason: "超时".to_string(),
        };
        let err: Error = db_err.into();
        let msg = err.to_string();
        assert!(msg.contains("数据库错误"));
        assert!(msg.contains("数据库连接失败"));
    }

    #[test]
    fn test_top_level_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let err: Error = io_err.into();
        let msg = err.to_string();
        assert!(msg.contains("IO错误"));
    }

    #[test]
    fn test_config_error_macro() {
        let err = config_error!(InvalidValue {
            field: "port".to_string(),
            value: "invalid".to_string(),
            reason: "必须是数字".to_string(),
        });
        let msg = err.to_string();
        assert!(msg.contains("配置错误"));
        assert!(msg.contains("无效的配置值"));
        assert!(msg.contains("port"));
    }

    #[test]
    fn test_file_error_macro() {
        let err = file_error!(AlreadyExists {
            path: PathBuf::from("test.txt"),
        });
        let msg = err.to_string();
        assert!(msg.contains("文件错误"));
        assert!(msg.contains("文件已存在"));
    }

    #[test]
    fn test_database_error_macro() {
        let err = database_error!(ConnectionFailed {
            host: "db.example.com".to_string(),
            port: 3306,
            reason: "网络不可达".to_string(),
        });
        let msg = err.to_string();
        assert!(msg.contains("数据库错误"));
        assert!(msg.contains("db.example.com:3306"));
    }

    #[test]
    fn test_parse_error_macro() {
        let err = parse_error!(InvalidNumber {
            value: "abc".to_string(),
            expected_type: "i64".to_string(),
        });
        let msg = err.to_string();
        assert!(msg.contains("解析错误"));
        assert!(msg.contains("无效的数值"));
    }

    #[test]
    fn test_export_error_macro() {
        let err = export_error!(SerializationFailed {
            data_type: "SqlLog".to_string(),
            reason: "包含无效字符".to_string(),
        });
        let msg = err.to_string();
        assert!(msg.contains("导出错误"));
        assert!(msg.contains("序列化失败"));
    }

    #[test]
    fn test_error_debug_format() {
        let err = ConfigError::NotFound(PathBuf::from("test.toml"));
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotFound"));
        assert!(debug_str.contains("test.toml"));
    }

    #[test]
    fn test_result_type_ok() {
        fn returns_ok() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(returns_ok().unwrap(), 42);
    }

    #[test]
    fn test_result_type_err() {
        fn returns_err() -> Result<i32> {
            Err(ConfigError::NoExporters.into())
        }
        assert!(returns_err().is_err());
    }

    #[test]
    fn test_error_chain() {
        fn inner() -> Result<()> {
            Err(ConfigError::NoExporters.into())
        }

        fn outer() -> Result<()> {
            inner()?;
            Ok(())
        }

        let result = outer();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("配置错误"));
    }
}

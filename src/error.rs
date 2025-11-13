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
    /// 没有配置任何导出器
    #[error("至少需要配置一个导出器 (database/csv/jsonl)")]
    NoExporters,
}

/// 文件操作错误
#[derive(Debug, Error)]
pub enum FileError {
    /// 文件已存在
    #[error("文件已存在: {path} (设置 overwrite=true 以覆盖)")]
    AlreadyExists { path: PathBuf },

    /// 文件写入失败
    #[error("写入文件失败 {path}: {reason}")]
    WriteFailed { path: PathBuf, reason: String },

    /// 创建目录失败
    #[error("创建目录失败 {path}: {reason}")]
    CreateDirectoryFailed { path: PathBuf, reason: String },
}

/// 数据库错误
#[derive(Debug, Error)]
pub enum DatabaseError {}

/// 解析错误
#[derive(Debug, Error)]
pub enum ParseError {}

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
}

/// 导出错误
#[derive(Debug, Error)]
pub enum ExportError {
    /// CSV 导出失败
    #[error("CSV导出失败 {path}: {reason}")]
    CsvExportFailed { path: PathBuf, reason: String },

    /// 序列化失败
    #[error("序列化失败 ({data_type}): {reason}")]
    SerializationFailed { data_type: String, reason: String },

    /// 创建输出文件失败
    #[error("创建输出文件失败 {path}: {reason}")]
    FileCreateFailed { path: PathBuf, reason: String },

    /// 写入文件失败
    #[error("写入文件失败 {path}: {reason}")]
    FileWriteFailed { path: PathBuf, reason: String },
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

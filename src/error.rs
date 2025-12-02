use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// 应用程序错误类型
#[derive(Debug, Error)]
pub enum Error {
    /// Configuration related error
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// File operation error
    #[error("File error: {0}")]
    File(#[from] FileError),

    /// Database operation error
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    /// Parse error
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    /// SQL log parser error
    #[error("SQL log parser error: {0}")]
    Parser(#[from] ParserError),

    /// Export error
    #[error("Export error: {0}")]
    Export(#[from] ExportError),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// 配置错误
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Configuration file not found
    #[error("Configuration file not found: {0}")]
    NotFound(PathBuf),

    /// Configuration file parse failed
    #[error("Failed to parse configuration file {path}: {reason}")]
    ParseFailed { path: PathBuf, reason: String },

    /// Invalid log level
    #[error("Invalid log level '{level}', valid values: {}", valid_levels.join(", "))]
    InvalidLogLevel {
        level: String,
        valid_levels: Vec<String>,
    },

    /// Invalid configuration value
    #[error("Invalid configuration value {field} = '{value}': {reason}")]
    InvalidValue {
        field: String,
        value: String,
        reason: String,
    },

    /// Missing required configuration: no exporters configured
    #[error("At least one exporter must be configured (database/csv)")]
    NoExporters,
}

/// 文件操作错误
#[derive(Debug, Error)]
pub enum FileError {
    /// File already exists
    #[error("File already exists: {path} (set overwrite=true to replace)")]
    AlreadyExists { path: PathBuf },

    /// File write failed
    #[error("Failed to write file {path}: {reason}")]
    WriteFailed { path: PathBuf, reason: String },

    /// Create directory failed
    #[error("Failed to create directory {path}: {reason}")]
    CreateDirectoryFailed { path: PathBuf, reason: String },
}

/// 数据库错误
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// Database export failed
    #[error("Database export failed ({table_name}): {reason}")]
    #[allow(dead_code)]
    DatabaseExportFailed { table_name: String, reason: String },
}

/// 解析错误
#[derive(Debug, Error)]
pub enum ParseError {}

/// SQL 日志解析器错误
#[derive(Debug, Error)]
pub enum ParserError {
    /// Path not found
    #[error("Path not found: {path}")]
    PathNotFound { path: PathBuf },

    /// Invalid path
    #[error("Invalid path {path}: {reason}")]
    InvalidPath { path: PathBuf, reason: String },

    /// Read directory failed
    #[error("Failed to read directory {path}: {reason}")]
    ReadDirFailed { path: PathBuf, reason: String },
}

/// 导出错误
#[derive(Debug, Error)]
pub enum ExportError {
    /// CSV export failed
    #[error("CSV export failed {path}: {reason}")]
    CsvExportFailed { path: PathBuf, reason: String },
    /// Failed to create output file
    #[error("Failed to create output file {path}: {reason}")]
    FileCreateFailed { path: PathBuf, reason: String },

    /// Failed to write file
    #[error("Failed to write file {path}: {reason}")]
    FileWriteFailed { path: PathBuf, reason: String },

    /// Database operation error
    #[error("Database error: {reason}")]
    DatabaseError { reason: String },
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

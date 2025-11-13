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
    #[error("至少需要配置一个导出器 (database/csv)")]
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
pub enum DatabaseError {
    /// 数据库导出失败
    #[error("数据库导出失败 ({table_name}): {reason}")]
    #[allow(dead_code)]
    DatabaseExportFailed { table_name: String, reason: String },
}

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

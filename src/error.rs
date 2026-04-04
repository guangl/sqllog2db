use std::io;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("File error: {0}")]
    File(#[from] FileError),

    #[error("SQL log parser error: {0}")]
    Parser(#[from] ParserError),

    #[error("Export error: {0}")]
    Export(#[from] ExportError),

    #[error("Update error: {0}")]
    Update(#[from] UpdateError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Interrupted by user")]
    Interrupted,
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Update failed: {0}")]
    UpdateFailed(String),

    #[error("Check for updates failed: {0}")]
    CheckFailed(String),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    NotFound(PathBuf),

    #[error("Failed to parse configuration file {path}: {reason}")]
    ParseFailed { path: PathBuf, reason: String },

    #[error("Invalid log level '{level}', valid values: {}", valid_levels.join(", "))]
    InvalidLogLevel {
        level: String,
        valid_levels: Vec<String>,
    },

    #[error("Invalid configuration value {field} = '{value}': {reason}")]
    InvalidValue {
        field: String,
        value: String,
        reason: String,
    },

    #[error("At least one exporter must be configured (csv/jsonl/sqlite)")]
    NoExporters,
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("File already exists: {path} (set overwrite=true to replace)")]
    AlreadyExists { path: PathBuf },

    #[error("Failed to write file {path}: {reason}")]
    WriteFailed { path: PathBuf, reason: String },

    #[error("Failed to create directory {path}: {reason}")]
    CreateDirectoryFailed { path: PathBuf, reason: String },
}

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("Path not found: {path}")]
    PathNotFound { path: PathBuf },

    #[error("Invalid path {path}: {reason}")]
    InvalidPath { path: PathBuf, reason: String },

    #[error("Failed to read directory {path}: {reason}")]
    ReadDirFailed { path: PathBuf, reason: String },
}

#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum ExportError {
    /// 文件写入失败（CSV、JSONL、错误日志等所有文件型导出器通用）
    #[error("Write failed {path}: {reason}")]
    WriteError { path: PathBuf, reason: String },

    /// `SQLite` 操作失败
    #[error("Database error: {reason}")]
    DatabaseError { reason: String },
}

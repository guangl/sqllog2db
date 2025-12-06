/// Error type tests
use dm_database_sqllog2db::error::*;
use std::path::PathBuf;

// ==================== ConfigError Tests ====================

#[test]
fn test_config_error_not_found() {
    let path = PathBuf::from("/nonexistent/config.toml");
    let error = ConfigError::NotFound(path.clone());
    let error_msg = format!("{error}");
    assert!(error_msg.contains("not found"));
}

#[test]
fn test_config_error_parse_failed() {
    let path = PathBuf::from("config.toml");
    let error = ConfigError::ParseFailed {
        path: path.clone(),
        reason: "invalid syntax".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("parse"));
    assert!(error_msg.contains("invalid syntax"));
}

#[test]
fn test_config_error_invalid_log_level() {
    let error = ConfigError::InvalidLogLevel {
        level: "invalid".to_string(),
        valid_levels: vec!["info".to_string(), "debug".to_string()],
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("invalid"));
    assert!(error_msg.contains("info"));
    assert!(error_msg.contains("debug"));
}

#[test]
fn test_config_error_invalid_value() {
    let error = ConfigError::InvalidValue {
        field: "retention_days".to_string(),
        value: "0".to_string(),
        reason: "must be > 0".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("retention_days"));
    assert!(error_msg.contains('0'));
    assert!(error_msg.contains("must be > 0"));
}

#[test]
fn test_config_error_no_exporters() {
    let error = ConfigError::NoExporters;
    let error_msg = format!("{error}");
    assert!(error_msg.contains("exporter"));
}

// ==================== FileError Tests ====================

#[test]
fn test_file_error_already_exists() {
    let path = PathBuf::from("output.csv");
    let error = FileError::AlreadyExists { path: path.clone() };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("already exists"));
    assert!(error_msg.contains("output.csv"));
}

#[test]
fn test_file_error_write_failed() {
    let path = PathBuf::from("readonly.txt");
    let error = FileError::WriteFailed {
        path: path.clone(),
        reason: "permission denied".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("write") || error_msg.contains("Write"));
    assert!(error_msg.contains("readonly.txt"));
    assert!(error_msg.contains("permission denied"));
}

#[test]
fn test_file_error_create_directory_failed() {
    let path = PathBuf::from("readonly/logs");
    let error = FileError::CreateDirectoryFailed {
        path: path.clone(),
        reason: "access denied".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("create") || error_msg.contains("Create"));
    assert!(error_msg.contains("access denied"));
}

// ==================== ParserError Tests ====================

#[test]
fn test_parser_error_path_not_found() {
    let path = PathBuf::from("nonexistent.log");
    let error = ParserError::PathNotFound { path: path.clone() };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("not found") || error_msg.contains("Not found"));
}

#[test]
fn test_parser_error_invalid_path() {
    let path = PathBuf::from("/some/path");
    let error = ParserError::InvalidPath {
        path: path.clone(),
        reason: "not a valid log file".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("Invalid") || error_msg.contains("invalid"));
    assert!(error_msg.contains("not a valid log file"));
}

#[test]
fn test_parser_error_read_dir_failed() {
    let path = PathBuf::from("/protected");
    let error = ParserError::ReadDirFailed {
        path: path.clone(),
        reason: "permission denied".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("permission denied"));
}

// ==================== Top-level Error Tests ====================

#[test]
fn test_error_from_config_error() {
    let config_err = ConfigError::NoExporters;
    let error: Error = Error::Config(config_err);
    let error_msg = format!("{error}");
    assert!(error_msg.contains("Configuration"));
}

#[test]
fn test_error_from_file_error() {
    let path = PathBuf::from("test.csv");
    let file_err = FileError::AlreadyExists { path };
    let error: Error = Error::File(file_err);
    let error_msg = format!("{error}");
    assert!(error_msg.contains("File") || error_msg.contains("file"));
}

#[test]
fn test_error_from_parser_error() {
    let path = PathBuf::from("test.log");
    let parser_err = ParserError::PathNotFound { path };
    let error: Error = Error::Parser(parser_err);
    let error_msg = format!("{error}");
    assert!(error_msg.contains("parser") || error_msg.contains("Parser"));
}

// ==================== ExportError Tests ====================

#[test]
fn test_csv_export_failed() {
    let error = ExportError::CsvExportFailed {
        path: PathBuf::from("output.csv"),
        reason: "invalid format".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("CSV") || error_msg.contains("csv"));
}

#[test]
fn test_file_create_failed() {
    let error = ExportError::FileCreateFailed {
        path: PathBuf::from("output.log"),
        reason: "permission denied".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("create") || error_msg.contains("Create"));
}

#[test]
fn test_file_write_failed() {
    let error = ExportError::FileWriteFailed {
        path: PathBuf::from("output.csv"),
        reason: "disk full".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("write") || error_msg.contains("Write"));
}

#[cfg(any(feature = "sqlite", feature = "duckdb", feature = "postgres"))]
#[test]
fn test_database_error() {
    let error = ExportError::DatabaseError {
        reason: "connection failed".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("Database") || error_msg.contains("database"));
}

#[cfg(feature = "dm")]
#[test]
fn test_io_error() {
    let error = ExportError::IoError {
        path: PathBuf::from("data.log"),
        reason: "read failed".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("IO") || error_msg.contains("io"));
}

#[cfg(feature = "dm")]
#[test]
fn test_external_tool_error() {
    let error = ExportError::ExternalToolError {
        tool: "sqlloader".to_string(),
        reason: "not found".to_string(),
    };
    let error_msg = format!("{error}");
    assert!(error_msg.contains("External") || error_msg.contains("external"));
    assert!(error_msg.contains("sqlloader"));
}

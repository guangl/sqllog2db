/// Comprehensive error handling tests
use dm_database_sqllog2db::error::{
    ConfigError, DatabaseError, Error, ExportError, FileError, ParseError, ParserError, Result,
};
use std::path::PathBuf;

// ==================== ConfigError Tests ====================

#[test]
fn test_config_error_not_found() {
    let path = PathBuf::from("missing_config.toml");
    let error = ConfigError::NotFound(path.clone());
    assert_eq!(
        error.to_string(),
        format!("Configuration file not found: {}", path.display())
    );
}

#[test]
fn test_config_error_parse_failed() {
    let path = PathBuf::from("bad_config.toml");
    let reason = "invalid TOML syntax".to_string();
    let error = ConfigError::ParseFailed {
        path: path.clone(),
        reason: reason.clone(),
    };
    assert!(
        error
            .to_string()
            .contains("Failed to parse configuration file")
    );
    assert!(error.to_string().contains("bad_config.toml"));
    assert!(error.to_string().contains("invalid TOML syntax"));
}

#[test]
fn test_config_error_invalid_log_level() {
    let error = ConfigError::InvalidLogLevel {
        level: "invalid_level".to_string(),
        valid_levels: vec!["trace".to_string(), "debug".to_string(), "info".to_string()],
    };
    assert!(error.to_string().contains("Invalid log level"));
    assert!(error.to_string().contains("invalid_level"));
    assert!(error.to_string().contains("trace"));
}

#[test]
fn test_config_error_invalid_value() {
    let error = ConfigError::InvalidValue {
        field: "timeout".to_string(),
        value: "not_a_number".to_string(),
        reason: "must be a positive integer".to_string(),
    };
    assert!(error.to_string().contains("Invalid configuration value"));
    assert!(error.to_string().contains("timeout"));
    assert!(error.to_string().contains("not_a_number"));
}

#[test]
fn test_config_error_no_exporters() {
    let error = ConfigError::NoExporters;
    assert_eq!(
        error.to_string(),
        "At least one exporter must be configured (database/csv)"
    );
}

// ==================== FileError Tests ====================

#[test]
fn test_file_error_already_exists() {
    let path = PathBuf::from("existing_file.csv");
    let error = FileError::AlreadyExists { path: path.clone() };
    assert!(error.to_string().contains("File already exists"));
    assert!(error.to_string().contains("existing_file.csv"));
}

#[test]
fn test_file_error_write_failed() {
    let path = PathBuf::from("read_only_file.csv");
    let reason = "Permission denied".to_string();
    let error = FileError::WriteFailed {
        path: path.clone(),
        reason: reason.clone(),
    };
    assert!(error.to_string().contains("Failed to write file"));
    assert!(error.to_string().contains("read_only_file.csv"));
    assert!(error.to_string().contains("Permission denied"));
}

#[test]
fn test_file_error_create_directory_failed() {
    let path = PathBuf::from("/invalid/path/to/create");
    let reason = "Permission denied".to_string();
    let error = FileError::CreateDirectoryFailed {
        path: path.clone(),
        reason: reason.clone(),
    };
    assert!(error.to_string().contains("Failed to create directory"));
    assert!(error.to_string().contains("Permission denied"));
}

// ==================== DatabaseError Tests ====================

#[test]
fn test_database_error_is_enum() {
    // DatabaseError is an empty enum
    let _type_check = std::any::type_name::<DatabaseError>();
}

// ==================== ParseError Tests ====================

#[test]
fn test_parse_error_is_enum() {
    // ParseError is an empty enum, we can test its existence
    let _type_check = std::any::type_name::<ParseError>();
}

// ==================== ParserError Tests ====================

#[test]
fn test_parser_error_path_not_found() {
    let path = PathBuf::from("sqllogs/missing.log");
    let error = ParserError::PathNotFound { path: path.clone() };
    assert!(error.to_string().contains("Path not found"));
    assert!(error.to_string().contains("missing.log"));
}

#[test]
fn test_parser_error_invalid_path() {
    let path = PathBuf::from("/proc/self/fd/10");
    let reason = "File descriptor, not a regular file".to_string();
    let error = ParserError::InvalidPath {
        path: path.clone(),
        reason: reason.clone(),
    };
    assert!(error.to_string().contains("Invalid path"));
    assert!(error.to_string().contains("File descriptor"));
}

#[test]
fn test_parser_error_read_dir_failed() {
    let path = PathBuf::from("restricted_dir");
    let reason = "Permission denied".to_string();
    let error = ParserError::ReadDirFailed {
        path: path.clone(),
        reason: reason.clone(),
    };
    assert!(error.to_string().contains("Failed to read"));
    assert!(error.to_string().contains("Permission denied"));
}

// ==================== ExportError Tests ====================

#[test]
fn test_export_error_is_enum() {
    // ExportError is an enum with various variants
    let _type_check = std::any::type_name::<ExportError>();
}

// ==================== Error Enum Tests ====================

#[test]
fn test_error_from_config_error() {
    let config_error = ConfigError::NoExporters;
    let error: Error = config_error.into();
    let error_string = error.to_string();
    assert!(error_string.contains("Configuration error"));
    assert!(error_string.contains("At least one exporter"));
}

#[test]
fn test_error_from_file_error() {
    let path = PathBuf::from("test.csv");
    let file_error = FileError::AlreadyExists { path };
    let error: Error = file_error.into();
    let error_string = error.to_string();
    assert!(error_string.contains("File error"));
}

#[test]
fn test_error_from_parser_error() {
    let path = PathBuf::from("missing.log");
    let parser_error = ParserError::PathNotFound { path };
    let error: Error = parser_error.into();
    let error_string = error.to_string();
    assert!(error_string.contains("SQL log parser error"));
}

#[test]
fn test_error_debug() {
    let path = PathBuf::from("test.log");
    let parser_error = ParserError::PathNotFound { path };
    let error: Error = parser_error.into();
    let debug_str = format!("{error:?}");
    assert!(debug_str.contains("Parser"));
}

// ==================== Result Type Tests ====================

#[test]
fn test_result_ok() {
    let result: Result<String> = Ok("success".to_string());
    assert!(result.is_ok());
    if let Ok(value) = result {
        assert_eq!(value, "success");
    }
}

#[test]
fn test_result_err() {
    let path = PathBuf::from("missing.log");
    let parser_error = ParserError::PathNotFound { path };
    let error: Error = parser_error.into();
    let result: Result<String> = Err(error);
    assert!(result.is_err());
}

// ==================== Error Type Conversion Tests ====================

#[test]
fn test_config_error_conversion_through_error_enum() {
    let config_error = ConfigError::NoExporters;
    let error = Error::from(config_error);

    match error {
        Error::Config(_) => (),
        _ => panic!("Expected Config error variant"),
    }
}

#[test]
fn test_multiple_error_type_handling() {
    let errors: Vec<Error> = vec![
        Error::Config(ConfigError::NoExporters),
        Error::File(FileError::AlreadyExists {
            path: PathBuf::from("test.csv"),
        }),
    ];

    assert_eq!(errors.len(), 2);
    for error in errors {
        assert!(!error.to_string().is_empty());
    }
}

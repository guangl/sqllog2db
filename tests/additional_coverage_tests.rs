/// Simple, focused additional tests to improve coverage
use dm_database_sqllog2db::config::*;
use dm_database_sqllog2db::constants::LOG_LEVELS;
use dm_database_sqllog2db::error::*;

// ==================== Constants Coverage ====================

#[test]
fn test_log_levels_completeness() {
    assert_eq!(LOG_LEVELS.len(), 5);
    assert!(LOG_LEVELS.contains(&"trace"));
    assert!(LOG_LEVELS.contains(&"debug"));
    assert!(LOG_LEVELS.contains(&"info"));
    assert!(LOG_LEVELS.contains(&"warn"));
    assert!(LOG_LEVELS.contains(&"error"));
}

// ==================== Error Type Display ====================

#[test]
fn test_config_error_no_exporters_display() {
    let error = ConfigError::NoExporters;
    assert_eq!(
        error.to_string(),
        "At least one exporter must be configured (database/csv)"
    );
}

#[test]
fn test_parser_error_path_not_found_display() {
    let path = std::path::PathBuf::from("test.log");
    let error = ParserError::PathNotFound { path };
    assert!(error.to_string().contains("Path not found"));
}

// ==================== Config Cloning ====================

#[test]
fn test_sqllog_config_clone_independence() {
    let config1 = SqllogConfig {
        directory: "test".to_string(),
    };
    let config2 = config1.clone();
    let config3 = config1.clone();

    assert_eq!(config1.directory, config2.directory);
    assert_eq!(config2.directory, config3.directory);
}

// ==================== Logging Config Retention Days ====================

#[test]
fn test_logging_config_large_retention() {
    let config = LoggingConfig {
        level: "info".to_string(),
        file: "app.log".to_string(),
        retention_days: 365,
    };
    assert_eq!(config.retention_days(), 365);
}

#[test]
fn test_logging_config_zero_retention() {
    let config = LoggingConfig {
        level: "info".to_string(),
        file: "app.log".to_string(),
        retention_days: 0,
    };
    assert_eq!(config.retention_days(), 0);
}

// ==================== CSV Exporter Configuration ====================

#[test]
fn test_csv_exporter_flag_combinations() {
    // Test overwrite only
    let exp1 = CsvExporter {
        file: "out1.csv".to_string(),
        overwrite: true,
        append: false,
    };
    assert!(exp1.overwrite && !exp1.append);

    // Test append only
    let exp2 = CsvExporter {
        file: "out2.csv".to_string(),
        overwrite: false,
        append: true,
    };
    assert!(!exp2.overwrite && exp2.append);

    // Test both flags
    let exp3 = CsvExporter {
        file: "out3.csv".to_string(),
        overwrite: true,
        append: true,
    };
    assert!(exp3.overwrite && exp3.append);
}

// ==================== Error Conversions ====================

#[test]
fn test_error_from_config_error() {
    let config_err = ConfigError::NoExporters;
    let error: Error = config_err.into();
    let msg = error.to_string();
    assert!(msg.contains("Configuration error"));
}

#[test]
fn test_error_from_file_error() {
    let file_err = FileError::AlreadyExists {
        path: std::path::PathBuf::from("test.csv"),
    };
    let error: Error = file_err.into();
    let msg = error.to_string();
    assert!(msg.contains("File error"));
}

#[test]
fn test_error_from_parser_error() {
    let parser_err = ParserError::PathNotFound {
        path: std::path::PathBuf::from("test.log"),
    };
    let error: Error = parser_err.into();
    let msg = error.to_string();
    assert!(msg.contains("SQL log parser error"));
}

// ==================== Debug Implementations ====================

#[test]
fn test_config_types_debug_impl() {
    let sqllog_cfg = SqllogConfig::default();
    let debug_str = format!("{sqllog_cfg:?}");
    assert!(debug_str.contains("SqllogConfig"));

    let error_cfg = ErrorConfig::default();
    let debug_str2 = format!("{error_cfg:?}");
    assert!(debug_str2.contains("ErrorConfig"));
}

// ==================== Path Validation Edge Cases ====================

#[test]
fn test_sqllog_config_validate_only_tabs() {
    let config = SqllogConfig {
        directory: "\t\t".to_string(),
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sqllog_config_validate_only_newlines() {
    let config = SqllogConfig {
        directory: "\n\n".to_string(),
    };
    assert!(config.validate().is_err());
}

// ==================== All Valid Log Levels ====================

#[test]
fn test_logging_config_validate_all_levels() {
    for level in LOG_LEVELS {
        let config = LoggingConfig {
            level: (*level).to_string(),
            file: "test.log".to_string(),
            retention_days: 7,
        };
        assert!(config.validate().is_ok(), "Level '{level}' should be valid");
    }
}

// ==================== Case Insensitivity in Levels ====================

#[test]
fn test_logging_config_level_case_insensitive() {
    // 日志级别验证是不区分大小写的
    let configs_valid = vec![
        "trace", "Trace", "TRACE", "TrAcE", "debug", "DEBUG", "Info", "INFO",
    ];

    for level in configs_valid {
        let config = LoggingConfig {
            level: level.to_string(),
            file: "test.log".to_string(),
            retention_days: 7,
        };
        assert!(
            config.validate().is_ok(),
            "Level '{level}' should be valid (case-insensitive)"
        );
    }
}

// ==================== Default Values Consistency ====================

#[test]
fn test_default_configs_consistency() {
    let cfg1 = LoggingConfig::default();
    let cfg2 = LoggingConfig::default();
    let cfg3 = LoggingConfig::default();

    assert_eq!(cfg1.level, cfg2.level);
    assert_eq!(cfg2.level, cfg3.level);
    assert_eq!(cfg1.file, cfg2.file);
    assert_eq!(cfg2.file, cfg3.file);
    assert_eq!(cfg1.retention_days, cfg2.retention_days);
}

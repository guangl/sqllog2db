/// Additional edge case and boundary tests to improve coverage
use dm_database_sqllog2db::config::*;
use dm_database_sqllog2db::constants::LOG_LEVELS;
use dm_database_sqllog2db::error::*;
use std::path::PathBuf;

// ==================== Constants Coverage Tests ====================

#[test]
fn test_log_levels_constant_completeness() {
    assert_eq!(LOG_LEVELS.len(), 5);
    assert!(LOG_LEVELS.contains(&"trace"));
    assert!(LOG_LEVELS.contains(&"debug"));
    assert!(LOG_LEVELS.contains(&"info"));
    assert!(LOG_LEVELS.contains(&"warn"));
    assert!(LOG_LEVELS.contains(&"error"));
}

#[test]
fn test_log_levels_ordering() {
    // Test that all valid log levels are accessible
    for level in LOG_LEVELS {
        assert!(!level.is_empty());
        assert!(level.chars().all(|c| c.is_ascii_lowercase()));
    }
}

#[test]
fn test_log_levels_iteration() {
    let mut count = 0;
    for _ in LOG_LEVELS {
        count += 1;
    }
    assert_eq!(count, 5);
}

// ==================== Error Display Tests ====================

#[test]
fn test_error_display_messages() {
    let errors: Vec<Box<dyn std::error::Error>> = vec![
        Box::new(ConfigError::NoExporters),
        Box::new(FileError::AlreadyExists {
            path: PathBuf::from("test.csv"),
        }),
    ];

    for error in errors {
        let msg = error.to_string();
        assert!(!msg.is_empty());
    }
}

// ==================== Config Default Implementations ====================

#[test]
fn test_sqllog_config_default_impl() {
    let config1 = SqllogConfig::default();
    let config2 = SqllogConfig::default();

    assert_eq!(config1.directory, config2.directory);
}

#[test]
fn test_error_config_default_impl() {
    let config1 = ErrorConfig::default();
    let config2 = ErrorConfig::default();

    assert_eq!(config1.file, config2.file);
}

#[test]
fn test_logging_config_default_impl() {
    let config1 = LoggingConfig::default();
    let config2 = LoggingConfig::default();

    assert_eq!(config1.level, config2.level);
    assert_eq!(config1.file, config2.file);
    assert_eq!(config1.retention_days, config2.retention_days);
}

// ==================== Debug Trait Implementation Tests ====================

#[test]
fn test_sqllog_config_debug() {
    let config = SqllogConfig::default();
    let debug_str = format!("{config:?}");
    assert!(debug_str.contains("SqllogConfig"));
    assert!(debug_str.contains("sqllogs"));
}

#[test]
fn test_error_config_debug() {
    let config = ErrorConfig::default();
    let debug_str = format!("{config:?}");
    assert!(debug_str.contains("ErrorConfig"));
}

#[test]
fn test_logging_config_debug() {
    let config = LoggingConfig::default();
    let debug_str = format!("{config:?}");
    assert!(debug_str.contains("LoggingConfig"));
}

// ==================== Config Clone Tests ====================

#[test]
fn test_sqllog_config_clone() {
    let config1 = SqllogConfig {
        directory: "test".to_string(),
    };
    let config2 = config1.clone();

    assert_eq!(config1.directory, config2.directory);
}

#[test]
fn test_error_config_clone() {
    let config1 = ErrorConfig {
        file: "test.log".to_string(),
    };
    let config2 = config1.clone();

    assert_eq!(config1.file, config2.file);
}

#[test]
fn test_logging_config_clone() {
    let config1 = LoggingConfig {
        level: "info".to_string(),
        file: "test.log".to_string(),
        retention_days: 7,
    };
    let config2 = config1.clone();

    assert_eq!(config1.level, config2.level);
    assert_eq!(config1.file, config2.file);
    assert_eq!(config1.retention_days, config2.retention_days);
}

// ==================== Validation Boundary Tests ====================

#[test]
fn test_logging_config_validate_each_valid_level() {
    for level in LOG_LEVELS {
        let config = LoggingConfig {
            level: (*level).to_string(),
            file: "test.log".to_string(),
            retention_days: 7,
        };
        assert!(config.validate().is_ok(), "Level {level} should be valid");
    }
}

#[test]
fn test_logging_config_validate_similar_invalid_levels() {
    let invalid_levels = vec![
        "trace ", // trailing space
        " trace", // leading space
        "tr ace", // space in middle
        "trac",   // incomplete
        "traces", // extra characters
    ];

    for level in invalid_levels {
        let config = LoggingConfig {
            level: level.to_string(),
            file: "test.log".to_string(),
            retention_days: 7,
        };
        assert!(
            config.validate().is_err(),
            "Level '{level}' should be invalid"
        );
    }

    // 测试大小写不敏感的有效级别
    let valid_case_variants = vec!["TRACE", "Trace", "TrAcE", "DEBUG", "Debug"];
    for level in valid_case_variants {
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

// ==================== Path Validation Boundary Tests ====================

#[test]
fn test_sqllog_config_validate_only_newline() {
    let config = SqllogConfig {
        directory: "\n".to_string(),
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sqllog_config_validate_only_tab() {
    let config = SqllogConfig {
        directory: "\t".to_string(),
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sqllog_config_validate_mixed_whitespace() {
    let config = SqllogConfig {
        directory: " \t \n ".to_string(),
    };
    assert!(config.validate().is_err());
}

// ==================== Accessor Method Tests ====================

#[test]
fn test_sqllog_config_directory_accessor_returns_str() {
    let config = SqllogConfig {
        directory: "test_path".to_string(),
    };
    let result = config.directory();
    assert_eq!(result, "test_path");
    assert!(std::ptr::eq(result.as_ptr(), config.directory.as_ptr()));
}

#[test]
fn test_error_config_file_accessor_returns_str() {
    let config = ErrorConfig {
        file: "error.log".to_string(),
    };
    let result = config.file();
    assert_eq!(result, "error.log");
}

#[test]
fn test_logging_config_level_accessor_returns_str() {
    let config = LoggingConfig {
        level: "debug".to_string(),
        file: "app.log".to_string(),
        retention_days: 7,
    };
    let result = config.level();
    assert_eq!(result, "debug");
}

#[test]
fn test_logging_config_file_accessor_returns_str() {
    let config = LoggingConfig {
        level: "info".to_string(),
        file: "app.log".to_string(),
        retention_days: 7,
    };
    let result = config.file();
    assert_eq!(result, "app.log");
}

#[test]
fn test_logging_config_retention_days_accessor() {
    let config = LoggingConfig {
        level: "info".to_string(),
        file: "app.log".to_string(),
        retention_days: 30,
    };
    assert_eq!(config.retention_days(), 30);
}

// ==================== Type Trait Tests ====================

#[test]
fn test_config_types_are_debug() {
    fn assert_is_debug<T: std::fmt::Debug>() {}

    assert_is_debug::<SqllogConfig>();
    assert_is_debug::<ErrorConfig>();
    assert_is_debug::<LoggingConfig>();
    assert_is_debug::<FeaturesConfig>();
}

#[test]
fn test_config_types_are_clone() {
    fn assert_is_clone<T: Clone>() {}

    assert_is_clone::<SqllogConfig>();
    assert_is_clone::<ErrorConfig>();
    assert_is_clone::<LoggingConfig>();
    assert_is_clone::<FeaturesConfig>();
}

// ==================== Error Type Trait Tests ====================

#[test]
fn test_error_types_are_debug() {
    fn assert_is_debug<T: std::fmt::Debug>() {}

    assert_is_debug::<ConfigError>();
    assert_is_debug::<FileError>();
    assert_is_debug::<ParserError>();
    assert_is_debug::<Error>();
}

// ==================== Must Use Attribute Tests ====================

#[test]
fn test_config_getter_methods_marked_must_use() {
    let config = SqllogConfig::default();
    // These should compile fine even if we don't use the return value
    // but in production code, Clippy would warn
    let _ = config.directory();
}

#[test]
fn test_directory_getter_lifetime() {
    let config = SqllogConfig {
        directory: "path".to_string(),
    };
    let dir_ref = config.directory();
    assert_eq!(dir_ref, "path");
}

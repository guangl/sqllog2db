//! 配置验证和加载测试
#[cfg(test)]
mod config_validation_tests {
    use dm_database_sqllog2db::config::{Config, LoggingConfig, SqllogConfig};
    use std::fs;
    use std::path::PathBuf;

    fn setup_test_dir(name: &str) -> PathBuf {
        let test_dir = PathBuf::from("target/test_config_validation").join(name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).expect("Failed to create test dir");
        test_dir
    }

    #[test]
    fn test_config_from_file_success() {
        let test_dir = setup_test_dir("valid_config");
        let config_file = test_dir.join("config.toml");

        let toml_content = r#"
[sqllog]
directory = "sqllogs"

[error]
file = "errors.jsonl"

[logging]
file = "app.log"
level = "info"
retention_days = 7

[features]

[exporter.csv]
file = "output.csv"
overwrite = true
append = false
"#;

        fs::write(&config_file, toml_content).unwrap();

        let result = Config::from_file(&config_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_from_file_not_found() {
        let test_dir = setup_test_dir("not_found");
        let config_file = test_dir.join("nonexistent.toml");

        let result = Config::from_file(&config_file);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            format!("{err:?}").contains("NotFound") || format!("{err:?}").contains("not found")
        );
    }

    #[test]
    fn test_config_from_str_parse_error() {
        let invalid_toml = "this is [[[not valid toml";

        let result = Config::from_str(invalid_toml, PathBuf::from("test.toml"));
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(format!("{err:?}").contains("ParseFailed") || format!("{err:?}").contains("parse"));
    }

    #[test]
    fn test_logging_config_validate_all_levels() {
        let levels = vec!["trace", "debug", "info", "warn", "error"];

        for level in levels {
            let config = LoggingConfig {
                file: "app.log".to_string(),
                level: level.to_string(),
                retention_days: 7,
            };

            assert!(config.validate().is_ok(), "Level {level} should be valid");
        }
    }

    #[test]
    fn test_logging_config_validate_invalid() {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "INVALID".to_string(),
            retention_days: 7,
        };

        assert!(
            config.validate().is_err(),
            "INVALID level should fail validation"
        );
    }

    #[test]
    fn test_sqllog_config_validate_empty_directory() {
        let config = SqllogConfig {
            directory: String::new(),
        };

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_sqllog_config_validate_whitespace_directory() {
        let config = SqllogConfig {
            directory: "   ".to_string(),
        };

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_sqllog_config_validate_valid_directory() {
        let config = SqllogConfig {
            directory: "sqllogs".to_string(),
        };

        let result = config.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_sqllog_config_directory_accessor() {
        let config = SqllogConfig {
            directory: "test_dir".to_string(),
        };

        assert_eq!(config.directory(), "test_dir");
    }

    #[test]
    fn test_config_default_values() {
        let config = Config::default();

        assert!(!config.sqllog.directory.is_empty());
        assert!(!config.error.file.is_empty());
        assert!(!config.logging.file.is_empty());
        assert!(!config.logging.level.is_empty());
    }

    #[test]
    fn test_config_validate_cascading() {
        let config = Config::default();

        // Should validate all sub-configs
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();

        assert_eq!(config.file, "logs/sqllog2db.log");
        assert_eq!(config.level, "info");
        assert_eq!(config.retention_days, 7);
    }

    #[test]
    fn test_sqllog_config_default() {
        let config = SqllogConfig::default();

        assert_eq!(config.directory, "sqllogs");
    }

    #[test]
    fn test_config_from_str_minimal() {
        let minimal_toml = r#"
[sqllog]
directory = "logs"

[error]
file = "err.jsonl"

[logging]
file = "app.log"
level = "debug"
retention_days = 30

[features]

[exporter.csv]
file = "output.csv"
overwrite = true
append = false
"#;

        let result = Config::from_str(minimal_toml, PathBuf::from("test.toml"));
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.sqllog.directory, "logs");
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.retention_days, 30);
    }

    #[test]
    fn test_config_clone() {
        let config = Config::default();
        let cloned = config.clone();

        assert_eq!(config.sqllog.directory, cloned.sqllog.directory);
        assert_eq!(config.logging.level, cloned.logging.level);
    }

    #[test]
    fn test_logging_config_clone() {
        let config = LoggingConfig::default();
        let cloned = config.clone();

        assert_eq!(config.file, cloned.file);
        assert_eq!(config.level, cloned.level);
        assert_eq!(config.retention_days, cloned.retention_days);
    }

    #[test]
    fn test_sqllog_config_clone() {
        let config = SqllogConfig::default();
        let cloned = config.clone();

        assert_eq!(config.directory, cloned.directory);
    }

    #[test]
    fn test_config_debug_format() {
        let config = Config::default();
        let debug_str = format!("{config:?}");

        assert!(debug_str.contains("Config"));
    }

    #[test]
    fn test_logging_config_debug_format() {
        let config = LoggingConfig::default();
        let debug_str = format!("{config:?}");

        assert!(debug_str.contains("LoggingConfig"));
    }

    #[test]
    fn test_sqllog_config_debug_format() {
        let config = SqllogConfig::default();
        let debug_str = format!("{config:?}");

        assert!(debug_str.contains("SqllogConfig"));
    }

    #[test]
    fn test_config_from_file_with_comments() {
        let test_dir = setup_test_dir("comments");
        let config_file = test_dir.join("config.toml");

        let toml_content = r#"
# This is a comment
[sqllog]
directory = "sqllogs"  # Another comment

[error]
file = "errors.jsonl"

[logging]
file = "app.log"
level = "info"
retention_days = 7

[features]

[exporter.csv]
file = "output.csv"
overwrite = true
append = false
"#;

        fs::write(&config_file, toml_content).unwrap();

        let result = Config::from_file(&config_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_validate_with_different_retention() {
        let mut config = Config::default();

        // Test with different retention values
        config.logging.retention_days = 1;
        let _ = config.validate();

        config.logging.retention_days = 365;
        let _ = config.validate();

        config.logging.retention_days = 0;
        let _ = config.validate();

        // Just verify it doesn't panic
    }
}

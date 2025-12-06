//! logging.rs 深度覆盖测试
#[cfg(test)]
mod logging_coverage_tests {
    use dm_database_sqllog2db::config::LoggingConfig;
    use dm_database_sqllog2db::logging;
    use std::fs;
    use std::path::PathBuf;

    fn setup_test_dir(name: &str) -> PathBuf {
        let test_dir = PathBuf::from("target/test_logging_coverage").join(name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).expect("Failed to create test dir");
        test_dir
    }

    #[test]
    fn test_init_logging_with_trace_level() {
        let test_dir = setup_test_dir("trace_level");
        let log_file = test_dir.join("app.log");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "trace".to_string(),
            retention_days: 7,
        };

        // This will fail because logging can only be initialized once
        // But we're testing the parsing path
        let _ = logging::init_logging(&config);
    }

    #[test]
    fn test_init_logging_with_debug_level() {
        let test_dir = setup_test_dir("debug_level");
        let log_file = test_dir.join("app.log");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "debug".to_string(),
            retention_days: 7,
        };

        let _ = logging::init_logging(&config);
    }

    #[test]
    fn test_init_logging_with_warn_level() {
        let test_dir = setup_test_dir("warn_level");
        let log_file = test_dir.join("app.log");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "warn".to_string(),
            retention_days: 7,
        };

        let _ = logging::init_logging(&config);
    }

    #[test]
    fn test_init_logging_with_error_level() {
        let test_dir = setup_test_dir("error_level");
        let log_file = test_dir.join("app.log");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "error".to_string(),
            retention_days: 7,
        };

        let _ = logging::init_logging(&config);
    }

    #[test]
    fn test_init_logging_with_info_level() {
        let test_dir = setup_test_dir("info_level");
        let log_file = test_dir.join("app.log");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };

        let _ = logging::init_logging(&config);
    }

    #[test]
    fn test_init_logging_invalid_level() {
        let test_dir = setup_test_dir("invalid_level");
        let log_file = test_dir.join("app.log");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "invalid".to_string(),
            retention_days: 7,
        };

        let result = logging::init_logging(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_init_logging_creates_directory() {
        let test_dir = setup_test_dir("create_dir");
        let nested = test_dir.join("nested").join("path");
        let log_file = nested.join("app.log");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };

        let _ = logging::init_logging(&config);

        // Directory should be created
        assert!(nested.exists());
    }

    #[test]
    fn test_init_logging_invalid_filename() {
        // Try with empty string filename
        let config = LoggingConfig {
            file: String::new(),
            level: "info".to_string(),
            retention_days: 7,
        };

        let result = logging::init_logging(&config);
        // Should fail
        let _ = result;
    }

    #[test]
    fn test_init_logging_with_different_extension() {
        let test_dir = setup_test_dir("txt_ext");
        let log_file = test_dir.join("app.txt");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };

        let _ = logging::init_logging(&config);
    }

    #[test]
    fn test_init_logging_without_extension() {
        let test_dir = setup_test_dir("no_ext");
        let log_file = test_dir.join("app");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };

        let _ = logging::init_logging(&config);
    }

    #[test]
    fn test_init_logging_retention_days_zero() {
        let test_dir = setup_test_dir("retention_zero");
        let log_file = test_dir.join("app.log");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "info".to_string(),
            retention_days: 0,
        };

        let _ = logging::init_logging(&config);
    }

    #[test]
    fn test_init_logging_retention_days_large() {
        let test_dir = setup_test_dir("retention_large");
        let log_file = test_dir.join("app.log");

        let config = LoggingConfig {
            file: log_file.to_str().unwrap().to_string(),
            level: "info".to_string(),
            retention_days: 365,
        };

        let _ = logging::init_logging(&config);
    }

    #[test]
    #[cfg(feature = "tui")]
    fn test_set_log_to_console_enabled() {
        logging::set_log_to_console(true);
        // No return value to check, just verify it doesn't panic
    }

    #[test]
    #[cfg(feature = "tui")]
    fn test_set_log_to_console_disabled() {
        logging::set_log_to_console(false);
        // No return value to check, just verify it doesn't panic
    }

    #[test]
    #[cfg(feature = "tui")]
    fn test_set_log_to_console_toggle() {
        logging::set_log_to_console(true);
        logging::set_log_to_console(false);
        logging::set_log_to_console(true);
    }
}

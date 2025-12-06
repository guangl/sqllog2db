//! 综合功能覆盖测试
#[cfg(test)]
mod comprehensive_coverage_tests {
    use dm_database_sqllog2db::config::{Config, LoggingConfig};
    use dm_database_sqllog2db::exporter::ExportStats;
    use std::fs;
    use std::path::PathBuf;

    fn setup_test_dir(name: &str) -> PathBuf {
        let test_dir = PathBuf::from("target/test_comprehensive").join(name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).expect("Failed to create test dir");
        test_dir
    }

    #[test]
    fn test_config_from_file_not_found() {
        let test_dir = setup_test_dir("config_not_found");
        let config_file = test_dir.join("nonexistent.toml");

        let result = Config::from_file(&config_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_from_str_invalid_toml() {
        let invalid_toml = "this is not valid toml {{{";
        let result = Config::from_str(invalid_toml, PathBuf::from("test.toml"));

        assert!(result.is_err());
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_logging_config_validate_invalid_level() {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "invalid_level".to_string(),
            retention_days: 7,
        };

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_logging_config_validate_trace() {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "trace".to_string(),
            retention_days: 7,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_logging_config_validate_debug() {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "debug".to_string(),
            retention_days: 7,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_logging_config_validate_info() {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_logging_config_validate_warn() {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "warn".to_string(),
            retention_days: 7,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_logging_config_validate_error() {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "error".to_string(),
            retention_days: 7,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_export_stats_new() {
        let stats = ExportStats::new();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn test_export_stats_record_success() {
        let mut stats = ExportStats::new();
        stats.record_success();
        assert_eq!(stats.exported, 1);
        assert_eq!(stats.total(), 1);
    }

    #[test]
    fn test_export_stats_record_skip() {
        let mut stats = ExportStats::new();
        stats.skipped += 1;
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.total(), 1);
    }

    #[test]
    fn test_export_stats_record_failure() {
        let mut stats = ExportStats::new();
        stats.failed += 1;
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.total(), 1);
    }

    #[test]
    fn test_export_stats_multiple_operations() {
        let mut stats = ExportStats::new();
        for _ in 0..10 {
            stats.record_success();
        }
        stats.skipped += 5;
        stats.failed += 3;

        assert_eq!(stats.exported, 10);
        assert_eq!(stats.skipped, 5);
        assert_eq!(stats.failed, 3);
        assert_eq!(stats.total(), 18);
    }

    #[test]
    fn test_export_stats_clone() {
        let mut stats = ExportStats::new();
        stats.record_success();
        stats.record_success();

        let cloned = stats.clone();
        assert_eq!(cloned.exported, 2);
        assert_eq!(cloned.total(), 2);
    }

    #[test]
    fn test_export_stats_debug() {
        let stats = ExportStats::new();
        let debug_str = format!("{stats:?}");
        assert!(debug_str.contains("ExportStats"));
    }

    #[test]
    fn test_config_validate_cascading() {
        let config = Config::default();

        // Should validate all sub-configs
        let result = config.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_export_stats_flush_operations() {
        let mut stats = ExportStats::new();

        // Directly modify flush_operations
        stats.flush_operations = 5;

        assert_eq!(stats.flush_operations, 5);
    }

    #[test]
    fn test_export_stats_last_flush_size() {
        let mut stats = ExportStats::new();

        stats.last_flush_size = 100;
        assert_eq!(stats.last_flush_size, 100);

        stats.last_flush_size = 200;
        assert_eq!(stats.last_flush_size, 200);
    }
}

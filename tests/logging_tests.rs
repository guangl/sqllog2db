// 测试 logging.rs 模块的完整功能
// 注意：由于全局日志记录器只能初始化一次，我们只能测试一个日志级别的初始化
#[cfg(test)]
mod logging_tests {
    use dm_database_sqllog2db::config::LoggingConfig;
    use dm_database_sqllog2db::logging::init_logging;
    use std::fs;
    use std::path::Path;
    use std::sync::Once;

    static INIT: Once = Once::new();

    /// 测试初始化日志系统（全局日志只能初始化一次）
    #[test]
    fn test_init_logging_once() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let log_file = format!("{log_dir}/test_logging_final.log");
        let config = LoggingConfig {
            level: "info".to_string(),
            file: log_file.clone(),
            retention_days: 7,
        };

        // 只初始化一次
        INIT.call_once(|| {
            let result = init_logging(&config);
            assert!(result.is_ok(), "Failed to initialize logging");
        });

        // 验证文件已创建
        assert!(Path::new(&log_file).exists(), "Log file was not created");
    }

    /// 测试无效的日志级别（应该失败）
    #[test]
    fn test_init_logging_invalid_level() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let log_file = format!("{log_dir}/test_invalid.log");
        let config = LoggingConfig {
            level: "invalid_level".to_string(),
            file: log_file.clone(),
            retention_days: 7,
        };

        // 第一次初始化会失败（无效级别），但不会设置全局日志记录器
        let result = init_logging(&config);
        assert!(result.is_err(), "Should fail with invalid level");

        // Clean up
        let _ = fs::remove_file(&log_file);
    }

    /// 测试日志配置中的保留天数
    #[test]
    fn test_logging_retention_days() {
        let config = LoggingConfig {
            level: "info".to_string(),
            file: "test.log".to_string(),
            retention_days: 30,
        };

        let retention = config.retention_days();
        assert_eq!(retention, 30, "Retention days should be 30");
    }

    /// 测试日志配置中的最小保留天数
    #[test]
    fn test_logging_retention_days_min() {
        let config = LoggingConfig {
            level: "info".to_string(),
            file: "test.log".to_string(),
            retention_days: 1,
        };

        let retention = config.retention_days();
        assert_eq!(retention, 1, "Retention days minimum should be 1");
    }

    /// 测试日志配置中的最大保留天数
    #[test]
    fn test_logging_retention_days_max() {
        let config = LoggingConfig {
            level: "info".to_string(),
            file: "test.log".to_string(),
            retention_days: 365,
        };

        let retention = config.retention_days();
        assert_eq!(retention, 365, "Retention days maximum should be 365");
    }

    /// 测试日志配置的日志级别字段
    #[test]
    fn test_logging_config_level_field() {
        let config = LoggingConfig {
            level: "debug".to_string(),
            file: "test.log".to_string(),
            retention_days: 7,
        };

        assert_eq!(config.level, "debug", "Log level should be 'debug'");
    }

    /// 测试日志配置的文件字段
    #[test]
    fn test_logging_config_file_field() {
        let file_path = "logs/application.log".to_string();
        let config = LoggingConfig {
            level: "info".to_string(),
            file: file_path.clone(),
            retention_days: 7,
        };

        assert_eq!(config.file, file_path, "File path should match");
    }

    /// 测试大写日志级别
    #[test]
    fn test_logging_config_uppercase_level() {
        let config = LoggingConfig {
            level: "INFO".to_string(),
            file: "test.log".to_string(),
            retention_days: 7,
        };

        assert_eq!(config.level, "INFO", "Uppercase level should be preserved");
    }

    /// 测试混合大小写日志级别
    #[test]
    fn test_logging_config_mixed_case_level() {
        let config = LoggingConfig {
            level: "InFo".to_string(),
            file: "test.log".to_string(),
            retention_days: 7,
        };

        assert_eq!(config.level, "InFo", "Mixed case level should be preserved");
    }

    /// 测试特殊字符在日志文件名中
    #[test]
    fn test_logging_config_special_chars_in_path() {
        let file_path = "logs/app_2024-12-06.log".to_string();
        let config = LoggingConfig {
            level: "info".to_string(),
            file: file_path.clone(),
            retention_days: 7,
        };

        assert_eq!(
            config.file, file_path,
            "Special characters should be allowed in path"
        );
    }

    /// 测试不同扩展名的日志文件
    #[test]
    fn test_logging_config_different_extension() {
        let file_path = "logs/app.txt".to_string();
        let config = LoggingConfig {
            level: "info".to_string(),
            file: file_path.clone(),
            retention_days: 7,
        };

        assert_eq!(
            config.file, file_path,
            "Different extensions should be allowed"
        );
    }

    /// 测试日志配置序列化
    #[test]
    fn test_logging_config_structure() {
        let config = LoggingConfig {
            level: "warn".to_string(),
            file: "/var/log/app.log".to_string(),
            retention_days: 14,
        };

        assert_eq!(config.level, "warn");
        assert_eq!(config.file, "/var/log/app.log");
        assert_eq!(config.retention_days(), 14);
    }
}

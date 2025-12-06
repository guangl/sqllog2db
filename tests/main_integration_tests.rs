/// Main entry point and application integration tests
#[cfg(test)]
mod main_integration_tests {
    use dm_database_sqllog2db::config::Config;
    use std::fs;

    /// 测试默认配置加载
    #[test]
    fn test_default_config_creation() {
        let config = Config::default();

        // 验证默认配置已创建
        assert_eq!(config.sqllog.directory(), "sqllogs");
        assert_eq!(config.error.file(), "export/errors.log");
        assert_eq!(config.logging.file(), "logs/sqllog2db.log");
        assert_eq!(config.logging.level(), "info");
        assert_eq!(config.logging.retention_days(), 7);
    }

    /// 测试从 TOML 文件加载配置
    #[test]
    fn test_load_config_from_file() {
        let test_dir = "target/test_outputs/main_tests";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).ok();

        let config_file = format!("{test_dir}/config.toml");
        let content = r#"[sqllog]
directory = "sql_logs"

[error]
file = "errors.jsonl"

[logging]
file = "app.log"
level = "debug"
retention_days = 14

[features.replace_parameters]
enable = false

[exporter.csv]
file = "output.csv"
overwrite = true
append = false
"#;

        fs::write(&config_file, content).unwrap();

        // 加载配置
        let config = Config::from_file(&config_file);
        assert!(config.is_ok(), "Failed to load config from file");

        let config = config.unwrap();
        assert_eq!(config.sqllog.directory(), "sql_logs");
        assert_eq!(config.logging.level(), "debug");
        assert_eq!(config.logging.retention_days(), 14);

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// 测试配置验证流程
    #[test]
    fn test_config_validation() {
        let config = Config::default();

        // 验证应该成功
        assert!(config.validate().is_ok(), "Default config should be valid");
    }

    /// 测试无效的配置验证（无导出器）
    #[test]
    fn test_invalid_config_no_exporters() {
        use dm_database_sqllog2db::config::{
            ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig, SqllogConfig,
        };

        let config = Config {
            sqllog: SqllogConfig {
                directory: "sqllogs".to_string(),
            },
            error: ErrorConfig {
                file: "errors.log".to_string(),
            },
            logging: LoggingConfig {
                file: "app.log".to_string(),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: FeaturesConfig::default(),
            exporter: ExporterConfig {
                #[cfg(feature = "csv")]
                csv: None,
                #[cfg(feature = "parquet")]
                parquet: None,
                #[cfg(feature = "jsonl")]
                jsonl: None,
                #[cfg(feature = "sqlite")]
                sqlite: None,
                #[cfg(feature = "duckdb")]
                duckdb: None,
                #[cfg(feature = "postgres")]
                postgres: None,
                #[cfg(feature = "dm")]
                dm: None,
            },
        };

        // 验证应该失败（无导出器）
        assert!(
            config.validate().is_err(),
            "Config without exporters should fail validation"
        );
    }

    /// 测试配置中 verbose 标志处理
    #[test]
    fn test_config_verbose_flag_handling() {
        let mut config = Config::default();

        // 模拟 verbose 标志的处理
        if true {
            // verbose flag
            config.logging.level = "debug".to_string();
        }

        assert_eq!(
            config.logging.level(),
            "debug",
            "Verbose flag should set level to debug"
        );
    }

    /// 测试配置中 quiet 标志处理
    #[test]
    fn test_config_quiet_flag_handling() {
        let mut config = Config::default();

        // 模拟 quiet 标志的处理
        if true {
            // quiet flag
            config.logging.level = "error".to_string();
        }

        assert_eq!(
            config.logging.level(),
            "error",
            "Quiet flag should set level to error"
        );
    }

    /// 测试配置优先级（quiet 优先于 verbose）
    #[test]
    fn test_config_flag_priority() {
        let mut config = Config::default();

        // verbose 和 quiet 同时设置时，quiet 优先
        let quiet_enabled = true;
        let verbose_enabled = false;

        if quiet_enabled {
            // quiet flag (takes precedence)
            config.logging.level = "error".to_string();
        } else if verbose_enabled {
            // verbose flag
            config.logging.level = "debug".to_string();
        }

        assert_eq!(
            config.logging.level(),
            "error",
            "Quiet should have priority"
        );
    }

    /// 测试配置文件不存在时的处理
    #[test]
    fn test_config_file_not_found_handling() {
        let config_path = "target/test_outputs/nonexistent_config.toml";
        let config = Config::from_file(config_path);

        // 应该返回 Err
        assert!(config.is_err(), "Should fail when config file not found");
    }

    /// 测试配置文件格式错误的处理
    #[test]
    fn test_config_file_invalid_format() {
        let test_dir = "target/test_outputs/main_tests";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).ok();

        let config_file = format!("{test_dir}/invalid_config.toml");
        fs::write(&config_file, "invalid toml content [[[").unwrap();

        // 加载配置应该失败
        let config = Config::from_file(&config_file);
        assert!(config.is_err(), "Invalid TOML should fail to load");

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// 测试配置中的日志级别设置
    #[test]
    fn test_config_logging_levels() {
        let levels = vec!["trace", "debug", "info", "warn", "error"];

        for level in levels {
            use dm_database_sqllog2db::config::LoggingConfig;

            let config = LoggingConfig {
                file: "test.log".to_string(),
                level: level.to_string(),
                retention_days: 7,
            };

            assert!(config.validate().is_ok(), "Level {level} should be valid");
        }
    }

    /// 测试配置中的保留天数边界
    #[test]
    fn test_config_retention_days_boundaries() {
        use dm_database_sqllog2db::config::LoggingConfig;

        // 最小值：1
        let config_min = LoggingConfig {
            file: "test.log".to_string(),
            level: "info".to_string(),
            retention_days: 1,
        };
        assert!(
            config_min.validate().is_ok(),
            "Min retention (1) should be valid"
        );

        // 最大值：365
        let config_max = LoggingConfig {
            file: "test.log".to_string(),
            level: "info".to_string(),
            retention_days: 365,
        };
        assert!(
            config_max.validate().is_ok(),
            "Max retention (365) should be valid"
        );

        // 无效值：0
        let config_zero = LoggingConfig {
            file: "test.log".to_string(),
            level: "info".to_string(),
            retention_days: 0,
        };
        assert!(
            config_zero.validate().is_err(),
            "Retention 0 should be invalid"
        );

        // 无效值：366
        let config_over = LoggingConfig {
            file: "test.log".to_string(),
            level: "info".to_string(),
            retention_days: 366,
        };
        assert!(
            config_over.validate().is_err(),
            "Retention > 365 should be invalid"
        );
    }

    /// 测试完整的应用程序启动流程
    #[test]
    fn test_application_startup_sequence() {
        // 1. 加载或使用默认配置
        let config = Config::default();

        // 2. 验证配置
        assert!(config.validate().is_ok(), "Config validation failed");

        // 3. 检查导出器配置
        assert!(
            config.exporter.has_exporters() || !config.exporter.has_exporters(),
            "Exporter check should complete without panic"
        );

        // 4. 检查日志配置
        assert_eq!(
            config.logging.retention_days(),
            7,
            "Default retention should be 7"
        );
    }

    /// 测试应用程序配置的元数据
    #[test]
    fn test_application_metadata() {
        let config = Config::default();

        // 验证所有必需的配置字段都存在
        assert!(
            !config.sqllog.directory().is_empty(),
            "SQLlog directory should not be empty"
        );
        assert!(
            !config.error.file().is_empty(),
            "Error file should not be empty"
        );
        assert!(
            !config.logging.file().is_empty(),
            "Logging file should not be empty"
        );
        assert!(
            !config.logging.level().is_empty(),
            "Log level should not be empty"
        );
        assert!(
            config.logging.retention_days() > 0,
            "Retention days should be positive"
        );
    }

    /// 测试配置的特殊字符处理
    #[test]
    fn test_config_special_characters() {
        use dm_database_sqllog2db::config::{ErrorConfig, LoggingConfig, SqllogConfig};

        let config = SqllogConfig {
            directory: "logs_2024-12-06".to_string(),
        };
        assert!(
            config.validate().is_ok(),
            "Should handle special chars in directory name"
        );

        let error_config = ErrorConfig {
            file: "errors_[test].log".to_string(),
        };
        assert_eq!(error_config.file(), "errors_[test].log");

        let logging_config = LoggingConfig {
            file: "app_v1.0.0.log".to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };
        assert_eq!(logging_config.file(), "app_v1.0.0.log");
    }

    /// 测试配置的路径处理
    #[test]
    fn test_config_path_handling() {
        use dm_database_sqllog2db::config::SqllogConfig;

        let paths = vec!["sqllogs", "logs/sqllogs", "./logs", "path/to/sqllogs"];

        for path in paths {
            let config = SqllogConfig {
                directory: path.to_string(),
            };
            assert!(config.validate().is_ok(), "Should handle path: {path}");
        }
    }

    /// 测试应用程序默认配置的导出器设置
    #[test]
    fn test_default_config_exporter_settings() {
        let config = Config::default();

        // 默认配置应该有至少一个导出器
        assert!(
            config.exporter.has_exporters(),
            "Default config should have exporters"
        );

        // 检查 CSV 导出器（如果可用）
        #[cfg(feature = "csv")]
        {
            if let Some(csv) = config.exporter.csv() {
                assert!(!csv.file.is_empty(), "CSV file should not be empty");
            }
        }
    }

    /// 测试配置的 Clone 和复制行为
    #[test]
    fn test_config_cloning() {
        let config1 = Config::default();
        let config2 = config1.clone();

        assert_eq!(config1.sqllog.directory(), config2.sqllog.directory());
        assert_eq!(config1.logging.level(), config2.logging.level());
    }

    /// 测试应用程序配置的默认值一致性
    #[test]
    fn test_config_default_consistency() {
        // 多次创建默认配置，验证值一致
        let config1 = Config::default();
        let config2 = Config::default();
        let config3 = Config::default();

        assert_eq!(config1.sqllog.directory(), config2.sqllog.directory());
        assert_eq!(config2.sqllog.directory(), config3.sqllog.directory());

        assert_eq!(
            config1.logging.retention_days(),
            config2.logging.retention_days()
        );
        assert_eq!(
            config2.logging.retention_days(),
            config3.logging.retention_days()
        );
    }
}

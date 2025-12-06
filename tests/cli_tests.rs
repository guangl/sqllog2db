// CLI-adjacent tests - testing publicly exported APIs that CLI uses
#[cfg(test)]
#[allow(clippy::needless_update)]
mod cli_integration_tests {
    use dm_database_sqllog2db::config::Config;
    use dm_database_sqllog2db::exporter::ExporterManager;
    use std::fs;

    /// 创建标准测试配置的 helper 函数
    fn create_basic_config(output_file: &str) -> Config {
        Config {
            sqllog: dm_database_sqllog2db::config::SqllogConfig {
                directory: "sqllogs".to_string(),
            },
            error: dm_database_sqllog2db::config::ErrorConfig {
                file: "errors.log".to_string(),
            },
            logging: dm_database_sqllog2db::config::LoggingConfig {
                file: "app.log".to_string(),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: dm_database_sqllog2db::config::FeaturesConfig::default(),
            exporter: dm_database_sqllog2db::config::ExporterConfig {
                csv: Some(dm_database_sqllog2db::config::CsvExporter {
                    file: output_file.to_string(),
                    overwrite: true,
                    append: false,
                }),
                ..Default::default()
            },
        }
    }

    /// Test that Configuration can be created and validated
    #[test]
    #[cfg(feature = "csv")]
    fn test_config_creation_for_cli() {
        let config = create_basic_config("output.csv");
        assert!(config.validate().is_ok());
    }

    /// Test config loaded from file can be validated
    #[test]
    #[cfg(feature = "csv")]
    fn test_config_loading_and_validation() {
        let test_dir = "target/test_outputs/cli_loading";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();

        let config_path = format!("{test_dir}/test_config.toml");
        let test_config = r#"[sqllog]
directory = "test_logs"

[error]
file = "error.log"

[logging]
file = "app.log"
level = "info"
retention_days = 7

[features.replace_parameters]
enable = false

[exporter.csv]
file = "output.csv"
overwrite = true
append = false
"#;

        fs::write(&config_path, test_config).unwrap();

        let config = Config::from_file(&config_path);
        assert!(config.is_ok(), "Failed to load config: {:?}", config.err());

        let config = config.unwrap();
        assert_eq!(config.sqllog.directory(), "test_logs");
        assert_eq!(config.logging.level(), "info");

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// Test `ExporterManager` with CLI-like workflow
    #[test]
    #[cfg(feature = "csv")]
    fn test_exporter_manager_cli_workflow() {
        let test_dir = "target/test_outputs/cli_exporter";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();

        let config = Config {
            sqllog: dm_database_sqllog2db::config::SqllogConfig {
                directory: test_dir.to_string(),
            },
            error: dm_database_sqllog2db::config::ErrorConfig {
                file: format!("{test_dir}/errors.log"),
            },
            logging: dm_database_sqllog2db::config::LoggingConfig {
                file: format!("{test_dir}/app.log"),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: dm_database_sqllog2db::config::FeaturesConfig::default(),
            exporter: dm_database_sqllog2db::config::ExporterConfig {
                csv: Some(dm_database_sqllog2db::config::CsvExporter {
                    file: format!("{test_dir}/export.csv"),
                    overwrite: true,
                    append: false,
                }),
                ..Default::default()
            },
        };

        // Simulate CLI workflow: create manager, initialize, finalize
        let mut manager =
            ExporterManager::from_config(&config).expect("Failed to create exporter manager");

        assert!(manager.initialize().is_ok(), "Failed to initialize");
        assert!(manager.finalize().is_ok(), "Failed to finalize");

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// Test config with verbose flag (would set logging.level = "debug")
    #[test]
    #[cfg(feature = "csv")]
    fn test_config_with_verbose_simulation() {
        let mut config = Config {
            sqllog: dm_database_sqllog2db::config::SqllogConfig {
                directory: "sqllogs".to_string(),
            },
            error: dm_database_sqllog2db::config::ErrorConfig {
                file: "errors.log".to_string(),
            },
            logging: dm_database_sqllog2db::config::LoggingConfig {
                file: "app.log".to_string(),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: dm_database_sqllog2db::config::FeaturesConfig::default(),
            exporter: dm_database_sqllog2db::config::ExporterConfig {
                csv: Some(dm_database_sqllog2db::config::CsvExporter {
                    file: "output.csv".to_string(),
                    overwrite: true,
                    append: false,
                }),
                ..Default::default()
            },
        };

        // Simulate CLI verbose flag behavior
        config.logging.level = "debug".to_string();
        assert_eq!(config.logging.level(), "debug");
    }

    /// Test config with quiet flag (would set logging.level = "error")
    #[test]
    #[cfg(feature = "csv")]
    fn test_config_with_quiet_simulation() {
        let mut config = Config {
            sqllog: dm_database_sqllog2db::config::SqllogConfig {
                directory: "sqllogs".to_string(),
            },
            error: dm_database_sqllog2db::config::ErrorConfig {
                file: "errors.log".to_string(),
            },
            logging: dm_database_sqllog2db::config::LoggingConfig {
                file: "app.log".to_string(),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: dm_database_sqllog2db::config::FeaturesConfig::default(),
            exporter: dm_database_sqllog2db::config::ExporterConfig {
                csv: Some(dm_database_sqllog2db::config::CsvExporter {
                    file: "output.csv".to_string(),
                    overwrite: true,
                    append: false,
                }),
                ..Default::default()
            },
        };

        // Simulate CLI quiet flag behavior
        config.logging.level = "error".to_string();
        assert_eq!(config.logging.level(), "error");
    }

    /// Test various logging levels
    #[test]
    fn test_valid_log_levels() {
        let test_dir = "target/test_outputs/cli_log_levels";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();

        let log_levels = vec!["trace", "debug", "info", "warn", "error"];

        for level in log_levels {
            let config_path = format!("{test_dir}/config_{level}.toml");
            let config_content = format!(
                r#"[sqllog]
directory = "sqllogs"

[error]
file = "error.log"

[logging]
file = "app.log"
level = "{level}"
retention_days = 7

[features.replace_parameters]
enable = false

[exporter.csv]
file = "output.csv"
overwrite = true
append = false
"#
            );

            fs::write(&config_path, config_content).unwrap();
            let config = Config::from_file(&config_path);
            assert!(
                config.is_ok(),
                "Failed to load config with log level '{}': {:?}",
                level,
                config.err()
            );

            let cfg = config.unwrap();
            assert_eq!(cfg.logging.level(), level);
        }

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// Test config file with different exporter settings
    #[test]
    #[cfg(feature = "csv")]
    fn test_config_with_different_csv_settings() {
        let test_dir = "target/test_outputs/cli_csv_settings";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();

        // Test with overwrite=true, append=false
        let config_path = format!("{test_dir}/config_overwrite.toml");
        let config_content = r#"[sqllog]
directory = "sqllogs"

[error]
file = "error.log"

[logging]
file = "app.log"
level = "info"
retention_days = 7

[features.replace_parameters]
enable = false

[exporter.csv]
file = "output.csv"
overwrite = true
append = false
"#;

        fs::write(&config_path, config_content).unwrap();
        let config = Config::from_file(&config_path).unwrap();

        if let Some(csv) = &config.exporter.csv {
            assert!(csv.overwrite);
            assert!(!csv.append);
        }

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// Test feature flag configuration
    #[test]
    #[cfg(feature = "csv")]
    fn test_config_feature_flags() {
        let config = Config {
            sqllog: dm_database_sqllog2db::config::SqllogConfig {
                directory: "sqllogs".to_string(),
            },
            error: dm_database_sqllog2db::config::ErrorConfig {
                file: "errors.log".to_string(),
            },
            logging: dm_database_sqllog2db::config::LoggingConfig {
                file: "app.log".to_string(),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: dm_database_sqllog2db::config::FeaturesConfig::default(),
            exporter: dm_database_sqllog2db::config::ExporterConfig {
                csv: Some(dm_database_sqllog2db::config::CsvExporter {
                    file: "output.csv".to_string(),
                    overwrite: true,
                    append: false,
                }),
                ..Default::default()
            },
        };

        // Test default feature state
        assert!(!config.features.should_replace_sql_parameters());
    }

    /// Test all config fields are populated correctly
    #[test]
    #[cfg(feature = "csv")]
    fn test_config_field_completeness() {
        let config = Config {
            sqllog: dm_database_sqllog2db::config::SqllogConfig {
                directory: "/path/to/logs".to_string(),
            },
            error: dm_database_sqllog2db::config::ErrorConfig {
                file: "/path/to/errors.log".to_string(),
            },
            logging: dm_database_sqllog2db::config::LoggingConfig {
                file: "/path/to/app.log".to_string(),
                level: "debug".to_string(),
                retention_days: 14,
            },
            features: dm_database_sqllog2db::config::FeaturesConfig::default(),
            exporter: dm_database_sqllog2db::config::ExporterConfig {
                csv: Some(dm_database_sqllog2db::config::CsvExporter {
                    file: "/path/to/output.csv".to_string(),
                    overwrite: false,
                    append: true,
                }),
                ..Default::default()
            },
        };

        // Verify all fields
        assert_eq!(config.sqllog.directory(), "/path/to/logs");
        assert_eq!(config.error.file, "/path/to/errors.log");
        assert_eq!(config.logging.file(), "/path/to/app.log");
        assert_eq!(config.logging.level(), "debug");
        assert_eq!(config.logging.retention_days(), 14);
        assert!(config.exporter.csv.is_some());
    }

    /// Test config validation catches invalid `retention_days`
    #[test]
    fn test_config_validation_retention_days() {
        let test_dir = "target/test_outputs/cli_retention";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();

        // Test invalid retention_days (0)
        let config_path = format!("{test_dir}/config_invalid.toml");
        let config_content = r#"[sqllog]
directory = "sqllogs"

[error]
file = "error.log"

[logging]
file = "app.log"
level = "info"
retention_days = 0

[features.replace_parameters]
enable = false

[exporter.csv]
file = "output.csv"
overwrite = true
append = false
"#;

        fs::write(&config_path, config_content).unwrap();
        let config = Config::from_file(&config_path);
        assert!(config.is_err(), "Should reject retention_days=0");

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// Test multiple config files loading
    #[test]
    #[cfg(feature = "csv")]
    fn test_multiple_config_files() {
        let test_dir = "target/test_outputs/cli_multi_config";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();

        for i in 1..=3 {
            let config_path = format!("{test_dir}/config_{i}.toml");
            let config_content = format!(
                r#"[sqllog]
directory = "logs_{i}"

[error]
file = "error_{i}.log"

[logging]
file = "app_{i}.log"
level = "info"
retention_days = 7

[features.replace_parameters]
enable = false

[exporter.csv]
file = "output_{i}.csv"
overwrite = true
append = false
"#
            );

            fs::write(&config_path, config_content).unwrap();
            let config = Config::from_file(&config_path).unwrap();
            assert_eq!(config.sqllog.directory(), format!("logs_{i}").as_str());
        }

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }
}

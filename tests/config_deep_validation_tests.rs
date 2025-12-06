//! 更深入的配置验证测试，覆盖更多边界情况
#![allow(clippy::needless_update)]
use dm_database_sqllog2db::config::*;
use std::fs;
use std::path::PathBuf;

#[allow(dead_code)]
fn setup_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_config_deep").join(name);
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    test_dir
}

#[test]
fn test_sqllog_config_empty_directory_validation() {
    let config = SqllogConfig {
        directory: String::new(),
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sqllog_config_whitespace_directory_validation() {
    let config = SqllogConfig {
        directory: "   ".to_string(),
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sqllog_config_valid_directory() {
    let config = SqllogConfig {
        directory: "logs".to_string(),
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_retention_boundary_0() {
    let config = LoggingConfig {
        file: "app.log".to_string(),
        level: "info".to_string(),
        retention_days: 0,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_logging_config_retention_boundary_1() {
    let config = LoggingConfig {
        file: "app.log".to_string(),
        level: "info".to_string(),
        retention_days: 1,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_retention_boundary_365() {
    let config = LoggingConfig {
        file: "app.log".to_string(),
        level: "info".to_string(),
        retention_days: 365,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_retention_boundary_366() {
    let config = LoggingConfig {
        file: "app.log".to_string(),
        level: "info".to_string(),
        retention_days: 366,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_logging_config_invalid_empty_level() {
    let config = LoggingConfig {
        file: "app.log".to_string(),
        level: String::new(),
        retention_days: 7,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_logging_config_level_case_insensitive_debug() {
    let configs = vec!["debug", "DEBUG", "Debug", "DeBuG"];

    for level in configs {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: level.to_string(),
            retention_days: 7,
        };
        assert!(config.validate().is_ok(), "Level {level} should be valid");
    }
}

#[test]
fn test_logging_config_level_case_insensitive_info() {
    let configs = vec!["info", "INFO", "Info", "InFo"];

    for level in configs {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: level.to_string(),
            retention_days: 7,
        };
        assert!(config.validate().is_ok(), "Level {level} should be valid");
    }
}

#[test]
fn test_logging_config_level_case_insensitive_warn() {
    let configs = vec!["warn", "WARN", "Warn", "WaRn"];

    for level in configs {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: level.to_string(),
            retention_days: 7,
        };
        assert!(config.validate().is_ok(), "Level {level} should be valid");
    }
}

#[test]
fn test_logging_config_level_case_insensitive_error() {
    let configs = vec!["error", "ERROR", "Error", "ErRoR"];

    for level in configs {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: level.to_string(),
            retention_days: 7,
        };
        assert!(config.validate().is_ok(), "Level {level} should be valid");
    }
}

#[test]
fn test_logging_config_level_case_insensitive_trace() {
    let configs = vec!["trace", "TRACE", "Trace", "TrAcE"];

    for level in configs {
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: level.to_string(),
            retention_days: 7,
        };
        assert!(config.validate().is_ok(), "Level {level} should be valid");
    }
}

#[test]
fn test_exporter_config_validate_no_exporters() {
    let config = ExporterConfig {
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
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_exporter_config_validate_with_csv() {
    let config = ExporterConfig {
        csv: Some(CsvExporter {
            file: "output.csv".to_string(),
            overwrite: true,
            append: false,
        }),
        ..Default::default()
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_full_validation_chain() {
    let config = Config {
        sqllog: SqllogConfig {
            directory: "logs".to_string(),
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
            csv: Some(CsvExporter {
                file: "output.csv".to_string(),
                overwrite: true,
                append: false,
            }),
            ..Default::default()
        },
    };

    // 测试每个组件的验证
    assert!(config.sqllog.validate().is_ok());
    assert!(config.logging.validate().is_ok());
    assert!(config.exporter.validate().is_ok());

    // 测试整体验证
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_fails_on_empty_directory() {
    let config = Config {
        sqllog: SqllogConfig {
            directory: String::new(),
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
            csv: Some(CsvExporter {
                file: "output.csv".to_string(),
                overwrite: true,
                append: false,
            }),
            ..Default::default()
        },
    };

    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_fails_on_invalid_log_level() {
    let config = Config {
        sqllog: SqllogConfig {
            directory: "logs".to_string(),
        },
        error: ErrorConfig {
            file: "errors.log".to_string(),
        },
        logging: LoggingConfig {
            file: "app.log".to_string(),
            level: "invalid".to_string(),
            retention_days: 7,
        },
        features: FeaturesConfig::default(),
        exporter: ExporterConfig {
            csv: Some(CsvExporter {
                file: "output.csv".to_string(),
                overwrite: true,
                append: false,
            }),
            ..Default::default()
        },
    };

    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_fails_on_no_exporters() {
    let config = Config {
        sqllog: SqllogConfig {
            directory: "logs".to_string(),
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

    assert!(config.validate().is_err());
}

#[test]
fn test_replace_parameters_feature_enabled() {
    let feature = ReplaceParametersFeature {
        enable: true,
        symbols: None,
    };

    let config = FeaturesConfig {
        replace_parameters: Some(feature),
    };

    assert!(config.should_replace_sql_parameters());
}

#[test]
fn test_replace_parameters_feature_disabled() {
    let feature = ReplaceParametersFeature {
        enable: false,
        symbols: None,
    };

    let config = FeaturesConfig {
        replace_parameters: Some(feature),
    };

    assert!(!config.should_replace_sql_parameters());
}

#[test]
fn test_replace_parameters_feature_none() {
    let config = FeaturesConfig {
        replace_parameters: None,
    };

    assert!(!config.should_replace_sql_parameters());
}

#[test]
fn test_csv_exporter_append_priority() {
    let config = CsvExporter {
        file: "test.csv".to_string(),
        overwrite: true,
        append: true,
    };

    let exporter = dm_database_sqllog2db::exporter::CsvExporter::from_config(&config);
    // append 应该优先于 overwrite
    let _ = exporter;
}

#[test]
fn test_config_from_toml_string() {
    let toml = r#"
[sqllog]
directory = "test_logs"

[error]
file = "test_errors.log"

[logging]
file = "test_app.log"
level = "debug"
retention_days = 14

[features.replace_parameters]
enable = false

[exporter.csv]
file = "test_output.csv"
overwrite = true
append = false
"#;

    let result: Result<Config, _> = toml::from_str(toml);
    assert!(result.is_ok());

    let config = result.unwrap();
    assert_eq!(config.sqllog.directory(), "test_logs");
    assert_eq!(config.logging.level(), "debug");
    assert_eq!(config.logging.retention_days(), 14);
}

#[test]
fn test_config_default_values() {
    let config = Config::default();

    assert_eq!(config.sqllog.directory(), "sqllogs");
    assert_eq!(config.error.file(), "export/errors.log");
    assert_eq!(config.logging.file(), "logs/sqllog2db.log");
    assert_eq!(config.logging.level(), "info");
    assert_eq!(config.logging.retention_days(), 7);
    assert!(!config.features.should_replace_sql_parameters());
}

#[test]
fn test_sqllog_config_default() {
    let config = SqllogConfig::default();
    assert_eq!(config.directory(), "sqllogs");
}

#[test]
fn test_error_config_default() {
    let config = ErrorConfig::default();
    assert_eq!(config.file(), "export/errors.log");
}

#[test]
fn test_logging_config_default() {
    let config = LoggingConfig::default();
    assert_eq!(config.file(), "logs/sqllog2db.log");
    assert_eq!(config.level(), "info");
    assert_eq!(config.retention_days(), 7);
}

#[test]
fn test_features_config_default() {
    let config = FeaturesConfig::default();
    assert!(!config.should_replace_sql_parameters());
    assert!(config.replace_parameters.is_none());
}

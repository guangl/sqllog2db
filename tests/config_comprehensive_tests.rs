/// Comprehensive configuration module tests with edge cases and validators
use dm_database_sqllog2db::config::*;
use std::path::PathBuf;

// ==================== SqllogConfig Advanced Tests ====================

#[test]
fn test_sqllog_config_with_relative_path() {
    let config = SqllogConfig {
        directory: "./logs/sqllogs".to_string(),
    };
    assert_eq!(config.directory(), "./logs/sqllogs");
}

#[test]
fn test_sqllog_config_with_absolute_path() {
    let config = SqllogConfig {
        directory: "/var/log/sqllogs".to_string(),
    };
    assert_eq!(config.directory(), "/var/log/sqllogs");
}

#[test]
fn test_sqllog_config_with_windows_path() {
    let config = SqllogConfig {
        directory: "C:\\Logs\\SqlLogs".to_string(),
    };
    assert_eq!(config.directory(), "C:\\Logs\\SqlLogs");
}

#[test]
fn test_sqllog_config_validate_whitespace_only() {
    let config = SqllogConfig {
        directory: "\t\n ".to_string(),
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sqllog_config_validate_single_space() {
    let config = SqllogConfig {
        directory: " ".to_string(),
    };
    assert!(config.validate().is_err());
}

// ==================== ErrorConfig Tests ====================

#[test]
fn test_error_config_default() {
    let config = ErrorConfig::default();
    assert_eq!(config.file(), "export/errors.log");
}

#[test]
fn test_error_config_custom_path() {
    let config = ErrorConfig {
        file: "/var/log/app-errors.log".to_string(),
    };
    assert_eq!(config.file(), "/var/log/app-errors.log");
}

#[test]
fn test_error_config_with_special_characters() {
    let config = ErrorConfig {
        file: "export/errors_2024-12-06.log".to_string(),
    };
    assert!(config.file().contains("2024"));
}

// ==================== LoggingConfig Advanced Tests ====================

#[test]
fn test_logging_config_default() {
    let config = LoggingConfig::default();
    assert_eq!(config.level(), "info");
    assert_eq!(config.file(), "logs/sqllog2db.log");
    assert_eq!(config.retention_days(), 7);
}

#[test]
fn test_logging_config_custom_retention() {
    let config = LoggingConfig {
        level: "debug".to_string(),
        file: "logs/app.log".to_string(),
        retention_days: 30,
    };
    assert_eq!(config.retention_days(), 30);
}

#[test]
fn test_logging_config_validate_trace_level() {
    let config = LoggingConfig {
        level: "trace".to_string(),
        file: "logs/app.log".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_debug_level() {
    let config = LoggingConfig {
        level: "debug".to_string(),
        file: "logs/app.log".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_warn_level() {
    let config = LoggingConfig {
        level: "warn".to_string(),
        file: "logs/app.log".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_error_level() {
    let config = LoggingConfig {
        level: "error".to_string(),
        file: "logs/app.log".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_invalid_level() {
    let config = LoggingConfig {
        level: "verbose".to_string(),
        file: "logs/app.log".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_logging_config_validate_empty_level() {
    let config = LoggingConfig {
        level: String::new(),
        file: "logs/app.log".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_logging_config_validate_mixed_case_level() {
    // 日志级别验证是不区分大小写的
    let config = LoggingConfig {
        level: "INFO".to_string(),
        file: "logs/app.log".to_string(),
        retention_days: 7,
    };
    assert!(
        config.validate().is_ok(),
        "Log level validation should be case-insensitive"
    );
}

#[test]
fn test_logging_config_zero_retention_days() {
    // 保留天数必须在 1-365 之间,0 是无效的
    let config = LoggingConfig {
        level: "info".to_string(),
        file: "logs/app.log".to_string(),
        retention_days: 0,
    };
    assert!(
        config.validate().is_err(),
        "Zero retention days should be invalid"
    );
}

#[test]
fn test_logging_config_large_retention_days() {
    let config = LoggingConfig {
        level: "info".to_string(),
        file: "logs/app.log".to_string(),
        retention_days: 365,
    };
    assert_eq!(config.retention_days(), 365);
}

// ==================== FeaturesConfig Tests ====================

#[test]
fn test_features_config_default() {
    let config = FeaturesConfig::default();
    assert!(!config.should_replace_sql_parameters());
}

#[test]
fn test_features_config_replace_parameters_disabled() {
    let config = FeaturesConfig {
        replace_parameters: Some(ReplaceParametersFeature {
            enable: false,
            symbols: None,
        }),
    };
    assert!(!config.should_replace_sql_parameters());
}

#[test]
fn test_features_config_replace_parameters_enabled() {
    let config = FeaturesConfig {
        replace_parameters: Some(ReplaceParametersFeature {
            enable: true,
            symbols: None,
        }),
    };
    assert!(config.should_replace_sql_parameters());
}

// ==================== CsvExporter Config Tests ====================

#[test]
fn test_csv_exporter_not_mutually_exclusive_with_append() {
    let exporter = CsvExporter {
        file: "output.csv".to_string(),
        overwrite: true,
        append: true,
    };
    // Both flags can technically be set (implementation will choose one)
    assert!(exporter.overwrite);
    assert!(exporter.append);
}

#[test]
fn test_csv_exporter_with_complex_filename() {
    let exporter = CsvExporter {
        file: "export/2024-12-06/sqllog_batch_001.csv".to_string(),
        overwrite: false,
        append: false,
    };
    assert!(exporter.file.contains("2024"));
    assert!(exporter.file.contains("csv"));
}

// ==================== ExporterConfig Tests ====================

#[test]
fn test_exporter_config_with_single_csv() {
    #[cfg(all(feature = "csv", not(any(feature = "parquet", feature = "jsonl"))))]
    {
        let exporter_config = ExporterConfig {
            csv: Some(CsvExporter {
                file: "output.csv".to_string(),
                overwrite: false,
                append: false,
            }),
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
        assert!(exporter_config.validate().is_ok());
    }

    #[cfg(feature = "csv")]
    {
        // Simple CSV-only test when CSV feature is enabled
        let exporter_config = ExporterConfig {
            csv: Some(CsvExporter {
                file: "output.csv".to_string(),
                overwrite: false,
                append: false,
            }),
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
        assert!(exporter_config.validate().is_ok());
    }
}

#[test]
fn test_exporter_config_no_exporters() {
    let exporter_config = ExporterConfig {
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
    assert!(exporter_config.validate().is_err());
}

#[test]
fn test_exporter_config_multiple_exporters() {
    #[cfg(all(feature = "csv", feature = "jsonl"))]
    {
        let exporter_config = ExporterConfig {
            csv: Some(CsvExporter {
                file: "output.csv".to_string(),
                overwrite: false,
                append: false,
            }),
            #[cfg(feature = "parquet")]
            parquet: None,
            jsonl: Some(JsonlExporter {
                file: "output.jsonl".to_string(),
                overwrite: false,
                append: false,
            }),
            #[cfg(feature = "sqlite")]
            sqlite: None,
            #[cfg(feature = "duckdb")]
            duckdb: None,
            #[cfg(feature = "postgres")]
            postgres: None,
            #[cfg(feature = "dm")]
            dm: None,
        };
        assert!(exporter_config.validate().is_ok());
    }
}

// ==================== Config Integration Tests ====================

#[test]
fn test_config_from_valid_toml_string() {
    let toml_str = r#"
[sqllog]
directory = "sqllogs"

[error]
file = "export/errors.log"

[logging]
level = "info"
file = "logs/sqllog2db.log"

[features]

[exporter]
[exporter.csv]
file = "export/output.csv"
overwrite = false
append = false
"#;

    let result = Config::from_str(toml_str, PathBuf::from("test.toml"));
    assert!(result.is_ok());
}

#[test]
fn test_config_validate_minimal_config() {
    let config = Config {
        sqllog: SqllogConfig {
            directory: "sqllogs".to_string(),
        },
        error: ErrorConfig {
            file: "export/errors.log".to_string(),
        },
        logging: LoggingConfig {
            level: "info".to_string(),
            file: "logs/sqllog2db.log".to_string(),
            retention_days: 7,
        },
        features: FeaturesConfig::default(),
        exporter: ExporterConfig {
            #[cfg(feature = "csv")]
            csv: Some(CsvExporter {
                file: "export/output.csv".to_string(),
                overwrite: false,
                append: false,
            }),
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

    let result = config.validate();
    assert!(result.is_ok());
}

#[test]
fn test_config_validate_fails_with_invalid_log_level_simple() {
    let config = Config {
        sqllog: SqllogConfig {
            directory: "sqllogs".to_string(),
        },
        error: ErrorConfig {
            file: "export/errors.log".to_string(),
        },
        logging: LoggingConfig {
            level: "INVALID".to_string(),
            file: "logs/sqllog2db.log".to_string(),
            retention_days: 7,
        },
        features: FeaturesConfig::default(),
        exporter: ExporterConfig {
            #[cfg(feature = "csv")]
            csv: Some(CsvExporter {
                file: "export/output.csv".to_string(),
                overwrite: false,
                append: false,
            }),
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

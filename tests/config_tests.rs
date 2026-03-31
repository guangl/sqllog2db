/// Configuration module tests
use dm_database_sqllog2db::config::*;
use dm_database_sqllog2db::features::ReplaceParametersFeature;

// ==================== SqllogConfig Tests ====================

#[test]
fn test_sqllog_config_default() {
    let config = SqllogConfig::default();
    assert_eq!(config.directory(), "sqllogs");
}

#[test]
fn test_sqllog_config_directory() {
    let config = SqllogConfig {
        directory: "custom/path".to_string(),
    };
    assert_eq!(config.directory(), "custom/path");
}

#[test]
fn test_sqllog_config_validate_success() {
    let config = SqllogConfig {
        directory: "sqllogs".to_string(),
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_sqllog_config_validate_empty_directory() {
    let config = SqllogConfig {
        directory: String::new(),
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sqllog_config_validate_whitespace_directory() {
    let config = SqllogConfig {
        directory: "   ".to_string(),
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
fn test_error_config_file() {
    let config = ErrorConfig {
        file: "logs/app_errors.log".to_string(),
    };
    assert_eq!(config.file(), "logs/app_errors.log");
}

// ==================== LoggingConfig Tests ====================

#[test]
fn test_logging_config_default() {
    let config = LoggingConfig::default();
    assert_eq!(config.file(), "logs/sqllog2db.log");
    assert_eq!(config.level(), "info");
    assert_eq!(config.retention_days(), 7);
}

#[test]
fn test_logging_config_getters() {
    let config = LoggingConfig {
        file: "custom.log".to_string(),
        level: "debug".to_string(),
        retention_days: 30,
    };
    assert_eq!(config.file(), "custom.log");
    assert_eq!(config.level(), "debug");
    assert_eq!(config.retention_days(), 30);
}

#[test]
fn test_logging_config_validate_valid_level_info() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "info".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_valid_level_debug() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "debug".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_valid_level_warn() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "warn".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_valid_level_error() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "error".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_case_insensitive_level() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "INFO".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_invalid_level() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "invalid".to_string(),
        retention_days: 7,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_logging_config_validate_retention_zero() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "info".to_string(),
        retention_days: 0,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_logging_config_validate_retention_too_large() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "info".to_string(),
        retention_days: 366,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_logging_config_validate_retention_valid_min() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "info".to_string(),
        retention_days: 1,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_logging_config_validate_retention_valid_max() {
    let config = LoggingConfig {
        file: "logs/app.log".to_string(),
        level: "info".to_string(),
        retention_days: 365,
    };
    assert!(config.validate().is_ok());
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
        filters: Some(FiltersFeature::default()),
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
        filters: Some(FiltersFeature::default()),
    };
    assert!(config.should_replace_sql_parameters());
}

#[test]
fn test_features_config_replace_parameters_enabled_with_symbols() {
    let config = FeaturesConfig {
        replace_parameters: Some(ReplaceParametersFeature {
            enable: true,
            symbols: Some(vec!["?".to_string(), "$".to_string()]),
        }),
        filters: Some(FiltersFeature::default()),
    };
    assert!(config.should_replace_sql_parameters());
}

// ==================== CSV Exporter Config Tests ====================

#[cfg(feature = "csv")]
#[test]
fn test_csv_exporter_default() {
    let exporter = CsvExporter::default();
    assert_eq!(exporter.file, "outputs/sqllog.csv");
    assert!(exporter.overwrite);
    assert!(!exporter.append);
}

#[cfg(feature = "csv")]
#[test]
fn test_csv_exporter_custom() {
    let exporter = CsvExporter {
        file: "custom.csv".to_string(),
        overwrite: false,
        append: true,
    };
    assert_eq!(exporter.file, "custom.csv");
    assert!(!exporter.overwrite);
    assert!(exporter.append);
}

// ==================== SQLite Exporter Config Tests ====================

#[cfg(feature = "sqlite")]
#[test]
fn test_sqlite_exporter_default() {
    let exporter = SqliteExporter::default();
    assert_eq!(exporter.database_url, "export/sqllog2db.db");
    assert_eq!(exporter.table_name, "sqllog_records");
    assert!(exporter.overwrite);
    assert!(!exporter.append);
}

#[cfg(feature = "sqlite")]
#[test]
fn test_sqlite_exporter_custom() {
    let exporter = SqliteExporter {
        database_url: "custom.db".to_string(),
        table_name: "custom_table".to_string(),
        overwrite: false,
        append: true,
    };
    assert_eq!(exporter.database_url, "custom.db");
    assert_eq!(exporter.table_name, "custom_table");
    assert!(!exporter.overwrite);
    assert!(exporter.append);
}

// ==================== JSONL Exporter Config Tests ====================

#[cfg(feature = "jsonl")]
#[test]
fn test_jsonl_exporter_default() {
    let exporter = JsonlExporter::default();
    assert_eq!(exporter.file, "export/sqllog2db.jsonl");
    assert!(exporter.overwrite);
    assert!(!exporter.append);
}

#[cfg(feature = "jsonl")]
#[test]
fn test_jsonl_exporter_custom() {
    let exporter = JsonlExporter {
        file: "custom.jsonl".to_string(),
        overwrite: false,
        append: true,
    };
    assert_eq!(exporter.file, "custom.jsonl");
    assert!(!exporter.overwrite);
    assert!(exporter.append);
}

// ==================== Exporter Config Tests ====================

#[test]
fn test_exporter_config_has_exporters_none() {
    let config = ExporterConfig {
        #[cfg(feature = "csv")]
        csv: None,
        #[cfg(feature = "jsonl")]
        jsonl: None,
        #[cfg(feature = "sqlite")]
        sqlite: None,
    };
    assert!(!config.has_exporters());
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_config_has_exporters_csv() {
    let config = ExporterConfig {
        csv: Some(CsvExporter::default()),
        #[cfg(feature = "jsonl")]
        jsonl: None,
        #[cfg(feature = "sqlite")]
        sqlite: None,
    };
    assert!(config.has_exporters());
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_config_total_exporters_one() {
    let config = ExporterConfig {
        csv: Some(CsvExporter::default()),
        #[cfg(feature = "jsonl")]
        jsonl: None,
        #[cfg(feature = "sqlite")]
        sqlite: None,
    };
    assert_eq!(config.total_exporters(), 1);
}

#[test]
fn test_exporter_config_validate_no_exporters() {
    let config = ExporterConfig {
        #[cfg(feature = "csv")]
        csv: None,
        #[cfg(feature = "jsonl")]
        jsonl: None,
        #[cfg(feature = "sqlite")]
        sqlite: None,
    };
    assert!(config.validate().is_err());
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_config_validate_with_csv() {
    let config = ExporterConfig {
        csv: Some(CsvExporter::default()),
        #[cfg(feature = "jsonl")]
        jsonl: None,
        #[cfg(feature = "sqlite")]
        sqlite: None,
    };
    assert!(config.validate().is_ok());
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_config_csv_getter() {
    let csv_exporter = CsvExporter::default();
    let config = ExporterConfig {
        csv: Some(csv_exporter),
        #[cfg(feature = "jsonl")]
        jsonl: None,
        #[cfg(feature = "sqlite")]
        sqlite: None,
    };
    assert!(config.csv().is_some());
    assert_eq!(config.csv().unwrap().file, "outputs/sqllog.csv");
}

#[cfg(feature = "sqlite")]
#[test]
fn test_exporter_config_sqlite_getter() {
    let sqlite_exporter = SqliteExporter::default();
    let config = ExporterConfig {
        #[cfg(feature = "csv")]
        csv: None,
        #[cfg(feature = "jsonl")]
        jsonl: None,
        sqlite: Some(sqlite_exporter),
    };
    assert!(config.sqlite().is_some());
    assert_eq!(config.sqlite().unwrap().database_url, "export/sqllog2db.db");
}

#[cfg(feature = "jsonl")]
#[test]
fn test_exporter_config_jsonl_getter() {
    let jsonl_exporter = JsonlExporter::default();
    let config = ExporterConfig {
        #[cfg(feature = "csv")]
        csv: None,
        jsonl: Some(jsonl_exporter),
        #[cfg(feature = "sqlite")]
        sqlite: None,
    };
    assert!(config.jsonl().is_some());
    assert_eq!(config.jsonl().unwrap().file, "export/sqllog2db.jsonl");
}

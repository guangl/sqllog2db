/// Configuration module tests
use dm_database_sqllog2db::config::*;

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

#[test]
fn test_features_config_replace_parameters_enabled_with_symbols() {
    let config = FeaturesConfig {
        replace_parameters: Some(ReplaceParametersFeature {
            enable: true,
            symbols: Some(vec!["?".to_string(), "$".to_string()]),
        }),
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

// ==================== DuckDB Exporter Config Tests ====================

#[cfg(feature = "duckdb")]
#[test]
fn test_duckdb_exporter_default() {
    let exporter = DuckdbExporter::default();
    assert_eq!(exporter.database_url, "export/sqllog2db.duckdb");
    assert_eq!(exporter.table_name, "sqllog_records");
    assert!(exporter.overwrite);
    assert!(!exporter.append);
}

#[cfg(feature = "duckdb")]
#[test]
fn test_duckdb_exporter_custom() {
    let exporter = DuckdbExporter {
        database_url: "custom.duckdb".to_string(),
        table_name: "logs".to_string(),
        overwrite: false,
        append: true,
    };
    assert_eq!(exporter.database_url, "custom.duckdb");
    assert_eq!(exporter.table_name, "logs");
    assert!(!exporter.overwrite);
    assert!(exporter.append);
}

// ==================== Parquet Exporter Config Tests ====================

#[cfg(feature = "parquet")]
#[test]
fn test_parquet_exporter_default() {
    let exporter = ParquetExporter::default();
    assert_eq!(exporter.file, "export/sqllog2db.parquet");
    assert!(exporter.overwrite);
    assert_eq!(exporter.row_group_size, Some(100_000));
    assert_eq!(exporter.use_dictionary, Some(true));
}

#[cfg(feature = "parquet")]
#[test]
fn test_parquet_exporter_custom() {
    let exporter = ParquetExporter {
        file: "custom.parquet".to_string(),
        overwrite: false,
        row_group_size: Some(50_000),
        use_dictionary: Some(false),
    };
    assert_eq!(exporter.file, "custom.parquet");
    assert!(!exporter.overwrite);
    assert_eq!(exporter.row_group_size, Some(50_000));
    assert_eq!(exporter.use_dictionary, Some(false));
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

// ==================== PostgreSQL Exporter Config Tests ====================

#[cfg(feature = "postgres")]
#[test]
fn test_postgres_exporter_default() {
    let exporter = PostgresExporter::default();
    assert_eq!(exporter.host, "localhost");
    assert_eq!(exporter.port, 5432);
    assert_eq!(exporter.username, "postgres");
    assert_eq!(exporter.password, "postgres");
    assert_eq!(exporter.database, "sqllog");
    assert_eq!(exporter.schema, "public");
    assert_eq!(exporter.table_name, "sqllog_records");
    assert!(exporter.overwrite);
    assert!(!exporter.append);
}

#[cfg(feature = "postgres")]
#[test]
fn test_postgres_exporter_connection_string_with_password() {
    let exporter = PostgresExporter {
        host: "db.example.com".to_string(),
        port: 5433,
        username: "user".to_string(),
        password: "pass123".to_string(),
        database: "mydb".to_string(),
        schema: "public".to_string(),
        table_name: "logs".to_string(),
        overwrite: true,
        append: false,
    };
    let conn_str = exporter.connection_string();
    assert!(conn_str.contains("host=db.example.com"));
    assert!(conn_str.contains("port=5433"));
    assert!(conn_str.contains("user=user"));
    assert!(conn_str.contains("password=pass123"));
    assert!(conn_str.contains("dbname=mydb"));
}

#[cfg(feature = "postgres")]
#[test]
fn test_postgres_exporter_connection_string_without_password() {
    let exporter = PostgresExporter {
        host: "localhost".to_string(),
        port: 5432,
        username: "postgres".to_string(),
        password: String::new(),
        database: "sqllog".to_string(),
        schema: "public".to_string(),
        table_name: "sqllog_records".to_string(),
        overwrite: true,
        append: false,
    };
    let conn_str = exporter.connection_string();
    assert!(conn_str.contains("host=localhost"));
    assert!(conn_str.contains("port=5432"));
    assert!(conn_str.contains("user=postgres"));
    assert!(!conn_str.contains("password"));
    assert!(conn_str.contains("dbname=sqllog"));
}

// ==================== DM Exporter Config Tests ====================

#[cfg(feature = "dm")]
#[test]
fn test_dm_exporter_default() {
    let exporter = DmExporter::default();
    assert_eq!(exporter.userid, "SYSDBA/SYSDBA@localhost:5236");
    assert_eq!(exporter.table_name, "sqllog_records");
    assert_eq!(exporter.control_file, "export/sqllog.ctl");
    assert_eq!(exporter.log_dir, "export/log");
}

#[cfg(feature = "dm")]
#[test]
fn test_dm_exporter_custom() {
    let exporter = DmExporter {
        userid: "user/pass@host:5236".to_string(),
        table_name: "custom_logs".to_string(),
        control_file: "custom.ctl".to_string(),
        log_dir: "custom_log".to_string(),
    };
    assert_eq!(exporter.userid, "user/pass@host:5236");
    assert_eq!(exporter.table_name, "custom_logs");
    assert_eq!(exporter.control_file, "custom.ctl");
    assert_eq!(exporter.log_dir, "custom_log");
}

// ==================== Exporter Config Tests ====================

#[test]
fn test_exporter_config_has_exporters_none() {
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
    assert!(!config.has_exporters());
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_config_has_exporters_csv() {
    let config = ExporterConfig {
        csv: Some(CsvExporter::default()),
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
    assert!(config.has_exporters());
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_config_total_exporters_one() {
    let config = ExporterConfig {
        csv: Some(CsvExporter::default()),
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
    assert_eq!(config.total_exporters(), 1);
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

#[cfg(feature = "csv")]
#[test]
fn test_exporter_config_validate_with_csv() {
    let config = ExporterConfig {
        csv: Some(CsvExporter::default()),
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
    assert!(config.validate().is_ok());
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_config_csv_getter() {
    let csv_exporter = CsvExporter::default();
    let config = ExporterConfig {
        csv: Some(csv_exporter),
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
        #[cfg(feature = "parquet")]
        parquet: None,
        #[cfg(feature = "jsonl")]
        jsonl: None,
        sqlite: Some(sqlite_exporter),
        #[cfg(feature = "duckdb")]
        duckdb: None,
        #[cfg(feature = "postgres")]
        postgres: None,
        #[cfg(feature = "dm")]
        dm: None,
    };
    assert!(config.sqlite().is_some());
    assert_eq!(config.sqlite().unwrap().database_url, "export/sqllog2db.db");
}

#[cfg(feature = "duckdb")]
#[test]
fn test_exporter_config_duckdb_getter() {
    let duckdb_exporter = DuckdbExporter::default();
    let config = ExporterConfig {
        #[cfg(feature = "csv")]
        csv: None,
        #[cfg(feature = "parquet")]
        parquet: None,
        #[cfg(feature = "jsonl")]
        jsonl: None,
        #[cfg(feature = "sqlite")]
        sqlite: None,
        duckdb: Some(duckdb_exporter),
        #[cfg(feature = "postgres")]
        postgres: None,
        #[cfg(feature = "dm")]
        dm: None,
    };
    assert!(config.duckdb().is_some());
    assert_eq!(
        config.duckdb().unwrap().database_url,
        "export/sqllog2db.duckdb"
    );
}

#[cfg(feature = "parquet")]
#[test]
fn test_exporter_config_parquet_getter() {
    let parquet_exporter = ParquetExporter::default();
    let config = ExporterConfig {
        #[cfg(feature = "csv")]
        csv: None,
        parquet: Some(parquet_exporter),
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
    assert!(config.parquet().is_some());
    assert_eq!(config.parquet().unwrap().file, "export/sqllog2db.parquet");
}

#[cfg(feature = "jsonl")]
#[test]
fn test_exporter_config_jsonl_getter() {
    let jsonl_exporter = JsonlExporter::default();
    let config = ExporterConfig {
        #[cfg(feature = "csv")]
        csv: None,
        #[cfg(feature = "parquet")]
        parquet: None,
        jsonl: Some(jsonl_exporter),
        #[cfg(feature = "sqlite")]
        sqlite: None,
        #[cfg(feature = "duckdb")]
        duckdb: None,
        #[cfg(feature = "postgres")]
        postgres: None,
        #[cfg(feature = "dm")]
        dm: None,
    };
    assert!(config.jsonl().is_some());
    assert_eq!(config.jsonl().unwrap().file, "export/sqllog2db.jsonl");
}

#[cfg(feature = "postgres")]
#[test]
fn test_exporter_config_postgres_getter() {
    let postgres_exporter = PostgresExporter::default();
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
        postgres: Some(postgres_exporter),
        #[cfg(feature = "dm")]
        dm: None,
    };
    assert!(config.postgres().is_some());
    assert_eq!(config.postgres().unwrap().host, "localhost");
}

#[cfg(feature = "dm")]
#[test]
fn test_exporter_config_dm_getter() {
    let dm_exporter = DmExporter::default();
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
        dm: Some(dm_exporter),
    };
    assert!(config.dm().is_some());
    assert_eq!(config.dm().unwrap().userid, "SYSDBA/SYSDBA@localhost:5236");
}

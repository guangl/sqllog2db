//! 针对 main.rs 和实际工作流的覆盖测试
#![allow(clippy::needless_update)]
use dm_database_sqllog2db::config::Config;
use std::fs;
use std::path::PathBuf;

fn setup_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_main_coverage").join(name);
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    test_dir
}

#[test]
fn test_config_load_and_validate_complete() {
    let test_dir = setup_test_dir("config_complete");
    let config_path = test_dir.join("config.toml");

    // 创建完整配置
    let content = r#"
[sqllog]
directory = "sqllogs"

[error]
file = "export/errors.log"

[logging]
file = "logs/app.log"
level = "info"
retention_days = 7

[features.replace_parameters]
enable = false

[exporter.csv]
file = "outputs/sqllog.csv"
overwrite = true
append = false
"#;

    fs::write(&config_path, content).expect("Failed to write config");

    // 测试加载
    let config = Config::from_file(&config_path);
    assert!(config.is_ok());

    let cfg = config.unwrap();
    assert!(cfg.validate().is_ok());
}

#[test]
fn test_config_load_verbose_quiet_flags() {
    let test_dir = setup_test_dir("flags");
    let config_path = test_dir.join("config.toml");

    let content = r#"
[sqllog]
directory = "sqllogs"

[error]
file = "errors.log"

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

    fs::write(&config_path, content).expect("Failed to write config");

    let mut config = Config::from_file(&config_path).unwrap();

    // 模拟 verbose flag
    config.logging.level = "debug".to_string();
    assert_eq!(config.logging.level(), "debug");

    // 模拟 quiet flag
    config.logging.level = "error".to_string();
    assert_eq!(config.logging.level(), "error");
}

#[test]
fn test_config_with_all_features_disabled() {
    let test_dir = setup_test_dir("features_disabled");
    let config_path = test_dir.join("config.toml");

    let content = r#"
[sqllog]
directory = "logs"

[error]
file = "errors.log"

[logging]
file = "app.log"
level = "warn"
retention_days = 30

[features.replace_parameters]
enable = false

[exporter.csv]
file = "out.csv"
overwrite = false
append = true
"#;

    fs::write(&config_path, content).expect("Failed to write config");

    let config = Config::from_file(&config_path).unwrap();
    assert!(!config.features.should_replace_sql_parameters());
}

#[test]
fn test_config_with_replace_parameters_enabled() {
    let test_dir = setup_test_dir("replace_enabled");
    let config_path = test_dir.join("config.toml");

    let content = r#"
[sqllog]
directory = "logs"

[error]
file = "errors.log"

[logging]
file = "app.log"
level = "info"
retention_days = 7

[features.replace_parameters]
enable = true
symbols = ["?", ":"]

[exporter.csv]
file = "out.csv"
overwrite = true
append = false
"#;

    fs::write(&config_path, content).expect("Failed to write config");

    let config = Config::from_file(&config_path).unwrap();
    assert!(config.features.should_replace_sql_parameters());
}

#[test]
fn test_exporter_config_has_exporters_check() {
    use dm_database_sqllog2db::config::{CsvExporter, ExporterConfig};

    // 有导出器
    let config_with = ExporterConfig {
        csv: Some(CsvExporter {
            file: "test.csv".to_string(),
            overwrite: true,
            append: false,
        }),
        ..Default::default()
    };
    assert!(config_with.has_exporters());

    // 无导出器
    let config_without = ExporterConfig {
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
    assert!(!config_without.has_exporters());
}

#[test]
fn test_exporter_config_csv_accessor() {
    use dm_database_sqllog2db::config::{CsvExporter, ExporterConfig};

    let csv_exporter = CsvExporter {
        file: "output.csv".to_string(),
        overwrite: true,
        append: false,
    };

    let config = ExporterConfig {
        csv: Some(csv_exporter.clone()),
        ..Default::default()
    };

    let csv = config.csv();
    assert!(csv.is_some());
    assert_eq!(csv.unwrap().file, "output.csv");
}

#[test]
fn test_features_config_default() {
    use dm_database_sqllog2db::config::FeaturesConfig;

    let features = FeaturesConfig::default();
    assert!(!features.should_replace_sql_parameters());
}

#[test]
fn test_sqllog_config_directory_accessor() {
    use dm_database_sqllog2db::config::SqllogConfig;

    let config = SqllogConfig {
        directory: "test_logs".to_string(),
    };

    assert_eq!(config.directory(), "test_logs");
}

#[test]
fn test_error_config_file_accessor() {
    use dm_database_sqllog2db::config::ErrorConfig;

    let config = ErrorConfig {
        file: "errors.jsonl".to_string(),
    };

    assert_eq!(config.file(), "errors.jsonl");
}

#[test]
fn test_logging_config_accessors() {
    use dm_database_sqllog2db::config::LoggingConfig;

    let config = LoggingConfig {
        file: "app.log".to_string(),
        level: "debug".to_string(),
        retention_days: 14,
    };

    assert_eq!(config.file(), "app.log");
    assert_eq!(config.level(), "debug");
    assert_eq!(config.retention_days(), 14);
}

#[test]
fn test_config_from_file_with_missing_fields() {
    let test_dir = setup_test_dir("missing_fields");
    let config_path = test_dir.join("config.toml");

    // 缺少必要字段
    let content = r#"
[sqllog]
directory = "logs"

[error]
file = "errors.log"
"#;

    fs::write(&config_path, content).expect("Failed to write config");

    let result = Config::from_file(&config_path);
    // 应该失败或使用默认值
    let _ = result;
}

#[test]
fn test_config_validation_with_invalid_exporter() {
    use dm_database_sqllog2db::config::{
        Config, ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig, SqllogConfig,
    };

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

    // 没有导出器应该验证失败
    assert!(config.validate().is_err());
}

#[test]
fn test_csv_exporter_config_modes() {
    use dm_database_sqllog2db::config::CsvExporter;

    // Overwrite mode
    let overwrite = CsvExporter {
        file: "test.csv".to_string(),
        overwrite: true,
        append: false,
    };
    assert!(overwrite.overwrite);
    assert!(!overwrite.append);

    // Append mode
    let append = CsvExporter {
        file: "test.csv".to_string(),
        overwrite: false,
        append: true,
    };
    assert!(!append.overwrite);
    assert!(append.append);

    // Normal mode
    let normal = CsvExporter {
        file: "test.csv".to_string(),
        overwrite: false,
        append: false,
    };
    assert!(!normal.overwrite);
    assert!(!normal.append);
}

#[test]
fn test_config_clone() {
    let config1 = Config::default();
    let config2 = config1.clone();

    assert_eq!(config1.sqllog.directory(), config2.sqllog.directory());
    assert_eq!(config1.logging.level(), config2.logging.level());
}

#[test]
fn test_exporter_manager_from_config() {
    use dm_database_sqllog2db::exporter::ExporterManager;

    let config = Config::default();
    let result = ExporterManager::from_config(&config);

    // 默认配置应该有导出器
    assert!(result.is_ok());
}

#[test]
fn test_parser_with_actual_log_files() {
    use dm_database_sqllog2db::parser::SqllogParser;

    let test_dir = setup_test_dir("parser_logs");

    // 创建测试日志文件
    fs::write(test_dir.join("test1.log"), "log content 1").unwrap();
    fs::write(test_dir.join("test2.log"), "log content 2").unwrap();
    fs::write(test_dir.join("test.txt"), "not a log").unwrap();

    let parser = SqllogParser::new(&test_dir);
    let files = parser.log_files().unwrap();

    // 应该只找到 .log 文件
    assert_eq!(files.len(), 2);
}

#[test]
fn test_complete_workflow_simulation() {
    use dm_database_sqllog2db::exporter::ExporterManager;
    use dm_database_sqllog2db::parser::SqllogParser;

    let test_dir = setup_test_dir("workflow");
    let csv_path = test_dir.join("output.csv");

    // 1. 创建配置
    let config_path = test_dir.join("config.toml");
    let csv_display = csv_path.to_string_lossy().to_string().replace('\\', "/");

    let content = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "errors.log"

[logging]
file = "app.log"
level = "info"
retention_days = 7

[features.replace_parameters]
enable = false

[exporter.csv]
file = "{}"
overwrite = true
append = false
"#,
        test_dir.to_string_lossy().replace('\\', "/"),
        csv_display
    );

    fs::write(&config_path, content).unwrap();

    // 2. 加载配置
    let config = Config::from_file(&config_path).unwrap();
    assert!(config.validate().is_ok());

    // 3. 创建解析器
    let parser = SqllogParser::new(&test_dir);
    let _ = parser.log_files();

    // 4. 创建导出器
    let mut manager = ExporterManager::from_config(&config).unwrap();
    assert!(manager.initialize().is_ok());
    assert!(manager.finalize().is_ok());

    // 5. 验证输出文件
    assert!(csv_path.exists());
}

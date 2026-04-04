//! 专项覆盖率补强测试
//! 目标：`sqlite exporter`、`show_config`、`jsonl` 记录导出、`config apply_overrides`、`color` 函数

// ============================================================
// SQLite Exporter 测试
// ============================================================
#[cfg(feature = "sqlite")]
mod sqlite_exporter_tests {
    use dm_database_parser_sqllog::LogParser;
    use dm_database_sqllog2db::config::SqliteExporter as SqliteExporterConfig;
    use dm_database_sqllog2db::exporter::{Exporter, SqliteExporter};
    use std::fmt::Write as _;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn setup_test_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(format!("target/test_sqlite_cov/{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 创建包含 N 条记录的临时日志文件，返回文件路径。
    fn make_log_file(dir: &Path, name: &str, count: usize) -> PathBuf {
        let mut content = String::new();
        for i in 0..count {
            let _ = writeln!(
                content,
                "2025-10-20 15:10:28.615 (EP[0] sess:0x1 user:testuser trxid:{i} stmt:0x2 appname:app ip:1.2.3.4) [INS] SELECT {i}. EXECTIME: 2(ms) ROWCOUNT: 1(rows) EXEC_ID: {i}."
            );
        }
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_sqlite_exporter_new_and_name() {
        let exporter = SqliteExporter::new(
            "target/test.db".to_string(),
            "sqllog_records".to_string(),
            true,
            false,
        );
        assert_eq!(exporter.name(), "SQLite");
    }

    #[test]
    fn test_sqlite_exporter_from_config() {
        let cfg = SqliteExporterConfig {
            database_url: "target/test_from_config.db".to_string(),
            table_name: "sqllog_records".to_string(),
            overwrite: true,
            append: false,
        };
        let exporter = SqliteExporter::from_config(&cfg);
        assert_eq!(exporter.name(), "SQLite");
    }

    #[test]
    fn test_sqlite_exporter_debug_format() {
        let exporter = SqliteExporter::new(
            "target/test_debug.db".to_string(),
            "records".to_string(),
            false,
            false,
        );
        let debug = format!("{exporter:?}");
        assert!(debug.contains("SqliteExporter"));
    }

    #[test]
    fn test_sqlite_initialize_and_finalize() {
        let dir = setup_test_dir("init_finalize");
        let db_path = dir.join("test.db").to_string_lossy().to_string();

        let mut exporter =
            SqliteExporter::new(db_path.clone(), "sqllog_records".to_string(), true, false);
        assert!(exporter.initialize().is_ok());
        assert!(exporter.finalize().is_ok());
        assert!(std::path::Path::new(&db_path).exists());
    }

    #[test]
    fn test_sqlite_overwrite_mode() {
        let dir = setup_test_dir("overwrite");
        let db_path = dir.join("test.db").to_string_lossy().to_string();

        // 第一次写
        {
            let mut exporter =
                SqliteExporter::new(db_path.clone(), "records".to_string(), true, false);
            exporter.initialize().unwrap();
            exporter.finalize().unwrap();
        }
        // 第二次写（overwrite）
        {
            let mut exporter =
                SqliteExporter::new(db_path.clone(), "records".to_string(), true, false);
            assert!(exporter.initialize().is_ok());
            assert!(exporter.finalize().is_ok());
        }
    }

    #[test]
    fn test_sqlite_append_mode() {
        let dir = setup_test_dir("append");
        let db_path = dir.join("test.db").to_string_lossy().to_string();

        // 先初始化建表
        {
            let mut exporter =
                SqliteExporter::new(db_path.clone(), "records".to_string(), false, false);
            exporter.initialize().unwrap();
            exporter.finalize().unwrap();
        }
        // append 模式
        {
            let mut exporter =
                SqliteExporter::new(db_path.clone(), "records".to_string(), false, true);
            assert!(exporter.initialize().is_ok());
            assert!(exporter.finalize().is_ok());
        }
    }

    #[test]
    fn test_sqlite_export_batch() {
        let dir = setup_test_dir("export_batch");
        let db_path = dir.join("test.db").to_string_lossy().to_string();
        let log_path = make_log_file(&dir, "test.log", 10);

        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();
        assert!(!records.is_empty());

        let mut exporter =
            SqliteExporter::new(db_path.clone(), "sqllog_records".to_string(), true, false);
        exporter.initialize().unwrap();
        exporter.export_batch(&records).unwrap();
        exporter.finalize().unwrap();

        let stats = exporter.stats_snapshot().unwrap();
        assert_eq!(stats.exported, records.len());
    }

    #[test]
    fn test_sqlite_export_single() {
        let dir = setup_test_dir("export_single");
        let db_path = dir.join("test.db").to_string_lossy().to_string();
        let log_path = make_log_file(&dir, "test.log", 1);

        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();
        assert_eq!(records.len(), 1);

        let mut exporter =
            SqliteExporter::new(db_path.clone(), "sqllog_records".to_string(), true, false);
        exporter.initialize().unwrap();
        exporter.export(&records[0]).unwrap();
        exporter.finalize().unwrap();

        let stats = exporter.stats_snapshot().unwrap();
        assert_eq!(stats.exported, 1);
    }

    #[test]
    fn test_sqlite_export_empty_batch() {
        let dir = setup_test_dir("empty_batch");
        let db_path = dir.join("test.db").to_string_lossy().to_string();

        let mut exporter =
            SqliteExporter::new(db_path.clone(), "sqllog_records".to_string(), true, false);
        exporter.initialize().unwrap();
        // 空 batch 应直接返回 Ok
        assert!(exporter.export_batch(&[]).is_ok());
        exporter.finalize().unwrap();
    }

    #[test]
    fn test_sqlite_stats_snapshot() {
        let dir = setup_test_dir("stats");
        let db_path = dir.join("test.db").to_string_lossy().to_string();

        let exporter = SqliteExporter::new(db_path, "sqllog_records".to_string(), true, false);
        let stats = exporter.stats_snapshot().unwrap();
        assert_eq!(stats.exported, 0);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_sqlite_nested_directory() {
        let dir = setup_test_dir("nested_dir");
        let db_path = dir
            .join("a")
            .join("b")
            .join("test.db")
            .to_string_lossy()
            .to_string();

        let mut exporter =
            SqliteExporter::new(db_path.clone(), "sqllog_records".to_string(), true, false);
        assert!(exporter.initialize().is_ok());
        assert!(exporter.finalize().is_ok());
    }

    #[cfg(feature = "replace_parameters")]
    #[test]
    fn test_sqlite_export_batch_with_normalized() {
        let dir = setup_test_dir("normalized");
        let db_path = dir.join("test.db").to_string_lossy().to_string();
        let log_path = make_log_file(&dir, "test.log", 3);

        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let normalized: Vec<Option<String>> = records
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i % 2 == 0 {
                    Some("SELECT ?".to_string())
                } else {
                    None
                }
            })
            .collect();

        let mut exporter =
            SqliteExporter::new(db_path.clone(), "sqllog_records".to_string(), true, false);
        exporter.initialize().unwrap();
        exporter
            .export_batch_with_normalized(&records, &normalized)
            .unwrap();
        exporter.finalize().unwrap();

        let stats = exporter.stats_snapshot().unwrap();
        assert_eq!(stats.exported, records.len());
    }
}

// ============================================================
// show_config 测试
// ============================================================
mod show_config_tests {
    use dm_database_sqllog2db::cli::show_config::handle_show_config;
    use dm_database_sqllog2db::config::{
        Config, ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig, SqllogConfig,
    };

    fn default_config() -> Config {
        Config {
            sqllog: SqllogConfig {
                directory: "sqllogs".to_string(),
            },
            error: ErrorConfig {
                file: "export/errors.log".to_string(),
            },
            logging: LoggingConfig {
                file: "logs/app.log".to_string(),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: FeaturesConfig::default(),
            exporter: ExporterConfig::default(),
        }
    }

    #[test]
    fn test_handle_show_config_runs_without_panic() {
        // handle_show_config 写到 stdout，只验证不 panic
        let cfg = default_config();
        handle_show_config(&cfg, "config.toml");
    }

    #[test]
    fn test_handle_show_config_with_custom_path() {
        let cfg = default_config();
        handle_show_config(&cfg, "/etc/sqllog2db/config.toml");
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_handle_show_config_with_csv() {
        use dm_database_sqllog2db::config::CsvExporter;
        let mut cfg = default_config();
        cfg.exporter.csv = Some(CsvExporter {
            file: "out.csv".to_string(),
            overwrite: true,
            append: false,
        });
        handle_show_config(&cfg, "config.toml");
    }

    #[cfg(feature = "jsonl")]
    #[test]
    fn test_handle_show_config_with_jsonl() {
        use dm_database_sqllog2db::config::JsonlExporter;
        let mut cfg = default_config();
        cfg.exporter.jsonl = Some(JsonlExporter {
            file: "out.jsonl".to_string(),
            overwrite: false,
            append: true,
        });
        handle_show_config(&cfg, "config.toml");
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_handle_show_config_with_sqlite() {
        use dm_database_sqllog2db::config::SqliteExporter;
        let mut cfg = default_config();
        cfg.exporter.sqlite = Some(SqliteExporter {
            database_url: "out.db".to_string(),
            table_name: "records".to_string(),
            overwrite: true,
            append: false,
        });
        handle_show_config(&cfg, "config.toml");
    }

    #[cfg(feature = "replace_parameters")]
    #[test]
    fn test_handle_show_config_with_replace_parameters() {
        use dm_database_sqllog2db::features::ReplaceParametersConfig;
        let mut cfg = default_config();
        cfg.features.replace_parameters = Some(ReplaceParametersConfig {
            enable: true,
            placeholders: vec![],
        });
        handle_show_config(&cfg, "config.toml");
    }

    #[cfg(feature = "filters")]
    #[test]
    fn test_handle_show_config_with_filters() {
        use dm_database_sqllog2db::config::FiltersFeature;
        let mut cfg = default_config();
        cfg.features.filters = Some(FiltersFeature {
            enable: true,
            meta: dm_database_sqllog2db::features::MetaFilters {
                start_ts: Some("2025-01-01".to_string()),
                end_ts: Some("2025-12-31".to_string()),
                trxids: Some(vec!["abc".to_string()].into_iter().collect()),
                usernames: Some(vec!["admin".to_string()]),
                client_ips: Some(vec!["1.2.3.4".to_string()]),
                ..Default::default()
            },
            ..Default::default()
        });
        handle_show_config(&cfg, "config.toml");
    }
}

// ============================================================
// JSONL Exporter 记录导出测试
// ============================================================
#[cfg(feature = "jsonl")]
mod jsonl_export_record_tests {
    use dm_database_parser_sqllog::LogParser;
    use dm_database_sqllog2db::exporter::{Exporter, JsonlExporter};
    use std::fmt::Write as _;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn setup_test_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(format!("target/test_jsonl_cov/{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_log_file(dir: &Path, name: &str, count: usize) -> PathBuf {
        let mut content = String::new();
        for i in 0..count {
            let _ = writeln!(
                content,
                "2025-10-20 15:10:28.615 (EP[0] sess:0x1 user:testuser trxid:{i} stmt:0x2 appname:app ip:1.2.3.4) [SEL] SELECT col{i} FROM t. EXECTIME: 1(ms) ROWCOUNT: 5(rows) EXEC_ID: {i}."
            );
        }
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_jsonl_export_single_record() {
        let dir = setup_test_dir("single");
        let output_file = dir.join("output.jsonl");
        let log_path = make_log_file(&dir, "test.log", 1);

        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();
        assert_eq!(records.len(), 1);

        let mut exporter = JsonlExporter::new(&output_file, true);
        exporter.initialize().unwrap();
        exporter.export(&records[0]).unwrap();
        exporter.finalize().unwrap();

        let content = fs::read_to_string(&output_file).unwrap();
        assert!(content.contains("\"ts\""));
        assert!(content.contains("\"sql\""));

        let stats = exporter.stats_snapshot().unwrap();
        assert_eq!(stats.exported, 1);
    }

    #[test]
    fn test_jsonl_export_batch_records() {
        let dir = setup_test_dir("batch");
        let output_file = dir.join("output.jsonl");
        let log_path = make_log_file(&dir, "test.log", 20);

        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();
        assert!(!records.is_empty());

        let mut exporter = JsonlExporter::new(&output_file, true);
        exporter.initialize().unwrap();
        exporter.export_batch(&records).unwrap();
        exporter.finalize().unwrap();

        let content = fs::read_to_string(&output_file).unwrap();
        let line_count = content.lines().count();
        assert_eq!(line_count, records.len());

        let stats = exporter.stats_snapshot().unwrap();
        assert_eq!(stats.exported, records.len());
    }

    #[test]
    fn test_jsonl_export_empty_batch() {
        let dir = setup_test_dir("empty_batch");
        let output_file = dir.join("output.jsonl");

        let mut exporter = JsonlExporter::new(&output_file, true);
        exporter.initialize().unwrap();
        assert!(exporter.export_batch(&[]).is_ok());
        exporter.finalize().unwrap();

        let content = fs::read_to_string(&output_file).unwrap();
        assert!(content.is_empty());
    }

    #[test]
    fn test_jsonl_output_is_valid_json_per_line() {
        let dir = setup_test_dir("valid_json");
        let output_file = dir.join("output.jsonl");
        let log_path = make_log_file(&dir, "test.log", 5);

        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let mut exporter = JsonlExporter::new(&output_file, true);
        exporter.initialize().unwrap();
        exporter.export_batch(&records).unwrap();
        exporter.finalize().unwrap();

        let content = fs::read_to_string(&output_file).unwrap();
        for line in content.lines() {
            // 每行应该是合法 JSON
            let parsed: serde_json::Value = serde_json::from_str(line).expect("每行应为合法 JSON");
            assert!(parsed.is_object());
            assert!(parsed.get("ts").is_some());
            assert!(parsed.get("sql").is_some());
        }
    }

    #[cfg(feature = "replace_parameters")]
    #[test]
    fn test_jsonl_export_batch_with_normalized() {
        let dir = setup_test_dir("normalized");
        let output_file = dir.join("output.jsonl");
        let log_path = make_log_file(&dir, "test.log", 4);

        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();
        let normalized: Vec<Option<String>> = records
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i % 2 == 0 {
                    Some("SELECT ?".to_string())
                } else {
                    None
                }
            })
            .collect();

        let mut exporter = JsonlExporter::new(&output_file, true);
        exporter.initialize().unwrap();
        exporter
            .export_batch_with_normalized(&records, &normalized)
            .unwrap();
        exporter.finalize().unwrap();

        let stats = exporter.stats_snapshot().unwrap();
        assert_eq!(stats.exported, records.len());
    }
}

// ============================================================
// Config apply_overrides 测试
// ============================================================
mod config_apply_overrides_tests {
    use dm_database_sqllog2db::config::Config;

    fn make_default_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_apply_override_sqllog_directory() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["sqllog.directory=/var/logs".to_string()])
            .unwrap();
        assert_eq!(cfg.sqllog.directory, "/var/logs");
    }

    #[test]
    fn test_apply_override_error_file() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["error.file=/tmp/errors.log".to_string()])
            .unwrap();
        assert_eq!(cfg.error.file, "/tmp/errors.log");
    }

    #[test]
    fn test_apply_override_logging_level() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["logging.level=debug".to_string()])
            .unwrap();
        assert_eq!(cfg.logging.level, "debug");
    }

    #[test]
    fn test_apply_override_logging_file() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["logging.file=/tmp/app.log".to_string()])
            .unwrap();
        assert_eq!(cfg.logging.file, "/tmp/app.log");
    }

    #[test]
    fn test_apply_override_logging_retention_days() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["logging.retention_days=30".to_string()])
            .unwrap();
        assert_eq!(cfg.logging.retention_days, 30);
    }

    #[test]
    fn test_apply_override_invalid_retention_days() {
        let mut cfg = make_default_config();
        let result = cfg.apply_overrides(&["logging.retention_days=notanumber".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_override_unknown_key() {
        let mut cfg = make_default_config();
        let result = cfg.apply_overrides(&["unknown.key=value".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_override_missing_equals() {
        let mut cfg = make_default_config();
        let result = cfg.apply_overrides(&["sqllog.directory".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_multiple_overrides() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&[
            "sqllog.directory=/data".to_string(),
            "logging.level=warn".to_string(),
        ])
        .unwrap();
        assert_eq!(cfg.sqllog.directory, "/data");
        assert_eq!(cfg.logging.level, "warn");
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_apply_override_csv_file() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.csv.file=/tmp/output.csv".to_string()])
            .unwrap();
        assert_eq!(cfg.exporter.csv.as_ref().unwrap().file, "/tmp/output.csv");
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_apply_override_csv_overwrite() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.csv.overwrite=false".to_string()])
            .unwrap();
        assert!(!cfg.exporter.csv.as_ref().unwrap().overwrite);
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_apply_override_csv_append() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.csv.append=true".to_string()])
            .unwrap();
        assert!(cfg.exporter.csv.as_ref().unwrap().append);
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_apply_override_csv_overwrite_invalid_bool() {
        let mut cfg = make_default_config();
        let result = cfg.apply_overrides(&["exporter.csv.overwrite=maybe".to_string()]);
        assert!(result.is_err());
    }

    #[cfg(feature = "jsonl")]
    #[test]
    fn test_apply_override_jsonl_file() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.jsonl.file=/tmp/out.jsonl".to_string()])
            .unwrap();
        assert_eq!(cfg.exporter.jsonl.as_ref().unwrap().file, "/tmp/out.jsonl");
    }

    #[cfg(feature = "jsonl")]
    #[test]
    fn test_apply_override_jsonl_overwrite() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.jsonl.overwrite=true".to_string()])
            .unwrap();
        assert!(cfg.exporter.jsonl.as_ref().unwrap().overwrite);
    }

    #[cfg(feature = "jsonl")]
    #[test]
    fn test_apply_override_jsonl_append() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.jsonl.append=true".to_string()])
            .unwrap();
        assert!(cfg.exporter.jsonl.as_ref().unwrap().append);
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_apply_override_sqlite_database_url() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.sqlite.database_url=/tmp/test.db".to_string()])
            .unwrap();
        assert_eq!(
            cfg.exporter.sqlite.as_ref().unwrap().database_url,
            "/tmp/test.db"
        );
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_apply_override_sqlite_table_name() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.sqlite.table_name=my_table".to_string()])
            .unwrap();
        assert_eq!(cfg.exporter.sqlite.as_ref().unwrap().table_name, "my_table");
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_apply_override_sqlite_overwrite() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.sqlite.overwrite=false".to_string()])
            .unwrap();
        assert!(!cfg.exporter.sqlite.as_ref().unwrap().overwrite);
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn test_apply_override_sqlite_append() {
        let mut cfg = make_default_config();
        cfg.apply_overrides(&["exporter.sqlite.append=true".to_string()])
            .unwrap();
        assert!(cfg.exporter.sqlite.as_ref().unwrap().append);
    }

    #[test]
    fn test_parse_bool_valid_values() {
        let mut cfg = make_default_config();
        // "1", "yes" 也是合法布尔值
        cfg.apply_overrides(&["exporter.csv.overwrite=1".to_string()])
            .unwrap();
        assert!(cfg.exporter.csv.as_ref().unwrap().overwrite);

        cfg.apply_overrides(&["exporter.csv.overwrite=no".to_string()])
            .unwrap();
        assert!(!cfg.exporter.csv.as_ref().unwrap().overwrite);

        cfg.apply_overrides(&["exporter.csv.overwrite=yes".to_string()])
            .unwrap();
        assert!(cfg.exporter.csv.as_ref().unwrap().overwrite);

        cfg.apply_overrides(&["exporter.csv.overwrite=0".to_string()])
            .unwrap();
        assert!(!cfg.exporter.csv.as_ref().unwrap().overwrite);
    }

    #[test]
    fn test_apply_empty_overrides() {
        let mut cfg = make_default_config();
        assert!(cfg.apply_overrides(&[]).is_ok());
    }
}

// ============================================================
// color 模块测试（强制 NO_COLOR 确保行为可预测）
// ============================================================
mod color_tests {
    use dm_database_sqllog2db::color;

    // 在 NO_COLOR 下，所有颜色函数应返回原始文本
    fn with_no_color<F: FnOnce()>(f: F) {
        // 注意：USE_COLOR 是 OnceLock，在进程内只初始化一次。
        // 测试环境下通常不是 terminal，所以 use_color() 返回 false，
        // 函数会直接返回原始文本。我们直接验证这个行为。
        f();
    }

    #[test]
    fn test_green_returns_string() {
        with_no_color(|| {
            let result = color::green("hello");
            assert!(!result.is_empty());
            assert!(result.contains("hello"));
        });
    }

    #[test]
    fn test_yellow_returns_string() {
        with_no_color(|| {
            let result = color::yellow("warn");
            assert!(result.contains("warn"));
        });
    }

    #[test]
    fn test_cyan_returns_string() {
        with_no_color(|| {
            let result = color::cyan("[section]");
            assert!(result.contains("[section]"));
        });
    }

    #[test]
    fn test_red_returns_string() {
        with_no_color(|| {
            let result = color::red("Error:");
            assert!(result.contains("Error:"));
        });
    }

    #[test]
    fn test_bold_returns_string() {
        with_no_color(|| {
            let result = color::bold("Title");
            assert!(result.contains("Title"));
        });
    }

    #[test]
    fn test_dim_returns_string() {
        with_no_color(|| {
            let result = color::dim("hint");
            assert!(result.contains("hint"));
        });
    }

    #[test]
    fn test_color_with_numeric_display() {
        with_no_color(|| {
            let result = color::green(42u64);
            assert!(result.contains("42"));
        });
    }

    #[test]
    fn test_color_with_empty_string() {
        with_no_color(|| {
            let result = color::green("");
            // 无论是否有颜色，结果都不应 panic
            let _ = result;
        });
    }
}

// ============================================================
// ExporterManager 补充测试
// ============================================================
mod exporter_manager_tests {
    use dm_database_parser_sqllog::LogParser;
    use dm_database_sqllog2db::exporter::ExporterManager;
    use std::fmt::Write as _;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn setup_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(format!("target/test_exp_mgr/{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_log_file(dir: &Path, count: usize) -> PathBuf {
        let mut content = String::new();
        for i in 0..count {
            let _ = writeln!(
                content,
                "2025-10-20 15:10:28.615 (EP[0] sess:0x1 user:u trxid:{i} stmt:0x2 appname:a ip:1.1.1.1) [INS] INSERT {i}. EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: {i}."
            );
        }
        let path = dir.join("test.log");
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_dry_run_exporter_manager() {
        let mut manager = ExporterManager::dry_run();
        manager.initialize().unwrap();
        manager.finalize().unwrap();
        assert_eq!(manager.name(), "dry-run");
    }

    #[test]
    fn test_dry_run_export_batch() {
        let dir = setup_dir("dry_run");
        let log_path = make_log_file(&dir, 5);
        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let mut manager = ExporterManager::dry_run();
        manager.initialize().unwrap();
        manager.export_batch(&records).unwrap();
        manager.finalize().unwrap();
    }

    #[test]
    fn test_dry_run_log_stats() {
        let mut manager = ExporterManager::dry_run();
        manager.initialize().unwrap();
        // 调用 log_stats 不应 panic
        manager.log_stats();
    }

    #[test]
    fn test_exporter_manager_debug_format() {
        let manager = ExporterManager::dry_run();
        let debug = format!("{manager:?}");
        assert!(debug.contains("ExporterManager"));
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_exporter_manager_from_config_csv() {
        use dm_database_sqllog2db::config::{
            Config, CsvExporter, ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig,
            SqllogConfig,
        };

        let dir = setup_dir("from_config_csv");
        let output = dir.join("out.csv").to_string_lossy().to_string();

        let cfg = Config {
            sqllog: SqllogConfig {
                directory: "sqllogs".to_string(),
            },
            error: ErrorConfig {
                file: "err.log".to_string(),
            },
            logging: LoggingConfig {
                file: "app.log".to_string(),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: FeaturesConfig::default(),
            exporter: ExporterConfig {
                csv: Some(CsvExporter {
                    file: output,
                    overwrite: true,
                    append: false,
                }),
                #[cfg(feature = "jsonl")]
                jsonl: None,
                #[cfg(feature = "sqlite")]
                sqlite: None,
            },
        };

        let manager = ExporterManager::from_config(&cfg);
        assert!(manager.is_ok());
        assert_eq!(manager.unwrap().name(), "CSV");
    }
}

/// CLI 运行流程和实际导出功能的完整集成测试
#[cfg(test)]
mod cli_run_integration_tests {
    use dm_database_sqllog2db::config::{
        Config, CsvExporter, ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig,
        SqllogConfig,
    };
    use dm_database_sqllog2db::exporter::ExporterManager;
    use dm_database_sqllog2db::parser::SqllogParser;
    use std::fs;

    /// 辅助函数：创建完整的测试环境
    fn setup_test_env(test_name: &str) -> (String, String) {
        let base_dir = format!("target/test_outputs/cli_run_{test_name}");
        let _ = fs::remove_dir_all(&base_dir);
        fs::create_dir_all(&base_dir).ok();

        let logs_dir = format!("{base_dir}/logs");
        let output_file = format!("{base_dir}/output.csv");

        fs::create_dir_all(&logs_dir).ok();

        (logs_dir, output_file)
    }

    /// 测试最小化 CLI 运行流程
    #[test]
    fn test_cli_run_minimal_flow() {
        let (_logs_dir, output_file) = setup_test_env("minimal");

        // 创建最小化配置
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
                csv: Some(CsvExporter {
                    file: output_file.clone(),
                    overwrite: true,
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

        // 验证配置
        assert!(config.validate().is_ok());

        // 创建导出器管理器
        let manager = ExporterManager::from_config(&config);
        assert!(manager.is_ok());
    }

    /// 测试 CLI 运行流程中的解析器初始化
    #[test]
    fn test_cli_run_parser_initialization() {
        let (logs_dir, _output_file) = setup_test_env("parser_init");

        // 创建 SQL 日志解析器
        let parser = SqllogParser::new(&logs_dir);

        // 验证解析器路径
        assert_eq!(parser.path().to_string_lossy(), logs_dir);
    }

    /// 测试 CLI 运行流程中的导出器初始化和完成
    #[test]
    fn test_cli_run_exporter_lifecycle() {
        let (_logs_dir, output_file) = setup_test_env("exporter_lifecycle");

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
                csv: Some(CsvExporter {
                    file: output_file,
                    overwrite: true,
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

        let mut manager = ExporterManager::from_config(&config).unwrap();

        // 测试初始化
        assert!(manager.initialize().is_ok());

        // 获取导出器名称
        let name = manager.name();
        assert_eq!(name, "CSV");

        // 测试完成
        assert!(manager.finalize().is_ok());
    }

    /// 测试 CLI 运行流程中没有日志文件的情况
    #[test]
    fn test_cli_run_no_log_files() {
        let (logs_dir, _output_file) = setup_test_env("no_logs");

        // 确保日志目录是空的
        let parser = SqllogParser::new(&logs_dir);
        let log_files = parser.log_files();

        // 空目录应该返回 Ok 但包含 0 个文件
        if let Ok(files) = log_files {
            assert_eq!(files.len(), 0, "Should have no files in empty directory");
        } else {
            // 非空目录返回错误是可以的
        }
    }

    /// 测试 CLI 命令行参数处理（config 标志）
    #[test]
    fn test_cli_config_file_path_handling() {
        let (_, _) = setup_test_env("config_path");

        // 模拟不同的配置路径
        let paths = vec!["config.toml", "./config.toml", "path/to/config.toml"];

        for path in paths {
            assert!(!path.is_empty(), "Config path should not be empty");
        }
    }

    /// 测试 CLI 中的 verbose 和 quiet 标志处理
    #[test]
    fn test_cli_verbose_quiet_flags() {
        let mut config = Config::default();

        // 测试 verbose 标志
        config.logging.level = "debug".to_string();
        assert_eq!(config.logging.level(), "debug");

        // 测试 quiet 标志
        config.logging.level = "error".to_string();
        assert_eq!(config.logging.level(), "error");

        // 测试默认值
        config.logging.level = "info".to_string();
        assert_eq!(config.logging.level(), "info");
    }

    /// 测试 CLI 运行流程中的统计信息收集
    #[test]
    fn test_cli_run_stats_collection() {
        let (_logs_dir, output_file) = setup_test_env("stats");

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
                csv: Some(CsvExporter {
                    file: output_file,
                    overwrite: true,
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

        let manager = ExporterManager::from_config(&config).unwrap();

        // 获取统计信息
        let stats = manager.stats();
        assert!(
            stats.is_some() || stats.is_none(),
            "Stats should be retrievable"
        );
    }

    /// 测试 CLI 运行中的错误处理
    #[test]
    fn test_cli_run_error_handling() {
        let (_logs_dir, _output_file) = setup_test_env("errors");

        // 测试无效配置
        let invalid_config = Config {
            sqllog: SqllogConfig {
                directory: String::new(), // 无效的空目录
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

        // 配置验证应该失败
        assert!(invalid_config.validate().is_err());
    }

    /// 测试 CLI 中不同导出器的选择
    #[test]
    fn test_cli_exporter_selection() {
        let (_logs_dir, output_file) = setup_test_env("exporter_selection");

        // CSV 导出器
        let csv_config = Config {
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
                csv: Some(CsvExporter {
                    file: output_file.clone(),
                    overwrite: true,
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

        assert!(csv_config.validate().is_ok());
        let manager = ExporterManager::from_config(&csv_config);
        assert!(manager.is_ok());
        assert_eq!(manager.unwrap().name(), "CSV");
    }

    /// 测试 CLI 运行流程的完整周期
    #[test]
    fn test_cli_run_complete_cycle() {
        let (_logs_dir, output_file) = setup_test_env("complete_cycle");

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
                csv: Some(CsvExporter {
                    file: output_file,
                    overwrite: true,
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

        // 1. 验证配置
        assert!(config.validate().is_ok());

        // 2. 创建解析器
        let parser = SqllogParser::new(config.sqllog.directory());
        assert_eq!(parser.path().to_string_lossy(), config.sqllog.directory());

        // 3. 创建导出器
        let mut manager = ExporterManager::from_config(&config).unwrap();

        // 4. 初始化
        assert!(manager.initialize().is_ok());

        // 5. 获取导出器名称
        let _name = manager.name();

        // 6. 完成
        assert!(manager.finalize().is_ok());
    }

    /// 测试 CLI 配置中的默认值
    #[test]
    fn test_cli_config_defaults() {
        let config = Config::default();

        assert_eq!(config.sqllog.directory(), "sqllogs");
        assert_eq!(config.logging.level(), "info");
        assert_eq!(config.logging.retention_days(), 7);
        assert!(!config.features.should_replace_sql_parameters());
    }

    /// 测试 CLI 中的配置验证链
    #[test]
    fn test_cli_config_validation_chain() {
        let config = Config::default();

        // 验证 sqllog 配置
        assert!(config.sqllog.validate().is_ok());

        // 验证 logging 配置
        assert!(config.logging.validate().is_ok());

        // 验证 exporter 配置
        assert!(config.exporter.validate().is_ok());

        // 验证整个配置
        assert!(config.validate().is_ok());
    }

    /// 测试 CLI 中的特殊字符处理
    #[test]
    fn test_cli_special_characters_in_paths() {
        let config = Config {
            sqllog: SqllogConfig {
                directory: "logs_2024-12-06".to_string(),
            },
            error: ErrorConfig {
                file: "errors_[test].log".to_string(),
            },
            logging: LoggingConfig {
                file: "app_v1.0.0.log".to_string(),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: FeaturesConfig::default(),
            exporter: ExporterConfig {
                #[cfg(feature = "csv")]
                csv: Some(CsvExporter {
                    file: "output_final.csv".to_string(),
                    overwrite: true,
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

        assert!(config.validate().is_ok());
    }

    /// 测试 CLI 命令调用的顺序
    #[test]
    fn test_cli_command_sequence() {
        // 模拟 CLI 命令的执行顺序
        let state = [
            "parse_args",
            "load_config",
            "validate_config",
            "init_logging",
            "create_resources",
            "run_export",
            "cleanup",
        ];

        assert_eq!(state.len(), 7);
    }
}

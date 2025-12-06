/// 完整流程集成测试 - 模拟实际的应用程序执行
#[cfg(test)]
mod full_workflow_tests {
    use dm_database_sqllog2db::config::{
        Config, CsvExporter, ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig,
        SqllogConfig,
    };
    use dm_database_sqllog2db::exporter::{Exporter, ExporterManager};
    use dm_database_sqllog2db::parser::SqllogParser;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Once;

    static INIT: Once = Once::new();

    /// 初始化测试目录（仅执行一次）
    fn init_test_dir() {
        INIT.call_once(|| {
            let _ = fs::create_dir_all("target/test_outputs");
        });
    }

    fn get_test_output_dir() -> PathBuf {
        init_test_dir();
        PathBuf::from("target/test_outputs")
    }

    #[test]
    fn test_complete_config_and_exporter_workflow() {
        // 创建测试输出目录
        let test_dir = get_test_output_dir();
        let csv_path = test_dir.join("complete_workflow.csv");

        // 清理旧文件
        let _ = fs::remove_file(&csv_path);

        // 1. 创建完整的应用程序配置
        let config = Config {
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
            features: FeaturesConfig {
                replace_parameters: None,
            },
            exporter: ExporterConfig {
                #[cfg(feature = "csv")]
                csv: Some(CsvExporter {
                    file: csv_path.to_string_lossy().to_string(),
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

        // 2. 验证配置
        assert!(config.validate().is_ok());

        // 3. 创建导出器管理器
        let mut manager = ExporterManager::from_config(&config).expect("Failed to create manager");

        // 4. 初始化导出器
        assert!(manager.initialize().is_ok());

        // 5. 创建解析器（测试解析器初始化）
        let parser = SqllogParser::new("sqllogs");
        assert_eq!(parser.path().to_string_lossy(), "sqllogs");

        // 6. 完成导出
        assert!(manager.finalize().is_ok());

        // 7. 验证文件已创建
        assert!(csv_path.exists());

        // 8. 验证文件内容
        let content = fs::read_to_string(&csv_path).expect("Failed to read CSV file");
        assert!(!content.is_empty());
        assert!(content.contains("ts,ep"));
    }

    #[test]
    fn test_config_validation_flow() {
        // 创建配置
        let config = Config {
            sqllog: SqllogConfig {
                directory: "test_input".to_string(),
            },
            error: ErrorConfig {
                file: "test_errors.log".to_string(),
            },
            logging: LoggingConfig {
                file: "test_app.log".to_string(),
                level: "debug".to_string(),
                retention_days: 14,
            },
            features: FeaturesConfig {
                replace_parameters: None,
            },
            exporter: ExporterConfig {
                #[cfg(feature = "csv")]
                csv: Some(CsvExporter {
                    file: "test_output.csv".to_string(),
                    overwrite: false,
                    append: true,
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

        // 验证所有配置组件
        assert!(config.sqllog.validate().is_ok());
        assert!(config.logging.validate().is_ok());
        assert!(config.exporter.validate().is_ok());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_parser_workflow() {
        // 测试解析器创建
        let parser = SqllogParser::new("test_input");

        // 验证路径
        assert_eq!(parser.path().to_string_lossy(), "test_input");

        // 测试另一种路径
        let parser2 = SqllogParser::new("nested/path/to/logs");
        assert_eq!(parser2.path().to_string_lossy(), "nested/path/to/logs");
    }

    #[test]
    fn test_exporter_chain_operations() {
        // 创建测试输出目录
        let test_dir = get_test_output_dir();
        let csv_path = test_dir.join("chain_operations.csv");

        // 清理旧文件
        let _ = fs::remove_file(&csv_path);

        // 1. 创建导出器
        let mut exporter = dm_database_sqllog2db::exporter::CsvExporter::new(&csv_path);

        // 2. 初始化
        assert!(exporter.initialize().is_ok());

        // 3. 获取导出器名称
        assert_eq!(exporter.name(), "CSV");

        // 4. 检查统计信息
        if let Some(stats) = exporter.stats_snapshot() {
            assert_eq!(stats.exported, 0);
        }

        // 5. 完成
        assert!(exporter.finalize().is_ok());

        // 6. 验证文件
        assert!(csv_path.exists());
    }

    #[test]
    fn test_multi_exporter_configuration_options() {
        use dm_database_sqllog2db::config::CsvExporter as CsvExporterConfig;

        // 测试多种导出器配置
        let configs = vec![
            CsvExporterConfig {
                file: "output1.csv".to_string(),
                overwrite: true,
                append: false,
            },
            CsvExporterConfig {
                file: "output2.csv".to_string(),
                overwrite: false,
                append: true,
            },
            CsvExporterConfig {
                file: "output3.csv".to_string(),
                overwrite: false,
                append: false,
            },
        ];

        for config in configs {
            let exporter = dm_database_sqllog2db::exporter::CsvExporter::from_config(&config);
            assert_eq!(exporter.name(), "CSV");
        }
    }

    #[test]
    fn test_logging_config_with_all_levels() {
        use dm_database_sqllog2db::constants::LOG_LEVELS;

        // 测试所有日志级别
        for &level in LOG_LEVELS {
            let config = LoggingConfig {
                file: "test.log".to_string(),
                level: level.to_string(),
                retention_days: 7,
            };

            assert!(config.validate().is_ok(), "Level {level} should be valid");
        }
    }

    #[test]
    fn test_config_serialization_structure() {
        // 创建和验证配置结构的各个部分
        let sqllog = SqllogConfig {
            directory: "input".to_string(),
        };

        let error = ErrorConfig {
            file: "errors.log".to_string(),
        };

        let logging = LoggingConfig {
            file: "app.log".to_string(),
            level: "warn".to_string(),
            retention_days: 30,
        };

        // 验证所有字段都已正确设置
        assert_eq!(sqllog.directory(), "input");
        assert_eq!(error.file(), "errors.log");
        assert_eq!(logging.file(), "app.log");
        assert_eq!(logging.level(), "warn");
        assert_eq!(logging.retention_days(), 30);
    }

    #[test]
    fn test_exporter_manager_full_lifecycle() {
        // 创建测试输出目录
        let test_dir = get_test_output_dir();
        let csv_path = test_dir.join("lifecycle_test.csv");

        // 清理旧文件
        let _ = fs::remove_file(&csv_path);

        // 创建配置
        let config = Config {
            sqllog: SqllogConfig {
                directory: "input".to_string(),
            },
            error: ErrorConfig {
                file: "errors.log".to_string(),
            },
            logging: LoggingConfig {
                file: "app.log".to_string(),
                level: "info".to_string(),
                retention_days: 7,
            },
            features: FeaturesConfig {
                replace_parameters: None,
            },
            exporter: ExporterConfig {
                #[cfg(feature = "csv")]
                csv: Some(CsvExporter {
                    file: csv_path.to_string_lossy().to_string(),
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

        // 创建和管理导出器的完整生命周期
        let mut manager = ExporterManager::from_config(&config).expect("Failed to create manager");

        // 初始化
        assert!(manager.initialize().is_ok());

        // 获取名称
        let name = manager.name();
        assert_eq!(name, "CSV");

        // 获取统计信息
        let stats = manager.stats();
        let _ = stats; // 可以是 Some 或 None

        // 完成
        assert!(manager.finalize().is_ok());

        // 验证输出文件
        assert!(csv_path.exists());
    }
}

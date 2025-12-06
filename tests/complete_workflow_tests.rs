// 高级集成测试，测试完整的导出工作流
#[cfg(test)]
#[allow(clippy::needless_update)]
mod complete_export_workflow_tests {
    use dm_database_sqllog2db::config::{
        Config, CsvExporter, ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig,
        SqllogConfig,
    };
    use dm_database_sqllog2db::exporter::ExporterManager;
    use std::fs;

    /// 创建测试配置（轻量级版本）
    fn create_test_config(output_file: &str) -> Config {
        Config {
            sqllog: SqllogConfig {
                directory: "target/test_outputs".to_string(),
            },
            error: ErrorConfig {
                file: "target/test_outputs/errors.jsonl".to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: "target/test_outputs/test.log".to_string(),
                retention_days: 7,
            },
            exporter: ExporterConfig {
                #[cfg(feature = "csv")]
                csv: Some(CsvExporter {
                    file: output_file.to_string(),
                    overwrite: true,
                    append: false,
                }),
                ..Default::default()
            },
            features: FeaturesConfig::default(),
        }
    }

    /// 测试完整工作流 - CSV 导出配置
    #[test]
    #[cfg(feature = "csv")]
    fn test_complete_workflow_csv_config() {
        let csv_file = "target/test_outputs/workflow_complete.csv";
        fs::create_dir_all("target/test_outputs").ok();

        let config = create_test_config(csv_file);

        // 验证配置结构有效
        assert_eq!(config.sqllog.directory, "target/test_outputs");
        assert_eq!(config.logging.level, "info");
        assert!(config.exporter.csv.is_some());

        let csv_exporter = config.exporter.csv.as_ref().unwrap();
        assert_eq!(csv_exporter.file, csv_file);

        // Clean up
        let _ = fs::remove_file(csv_file);
    }

    /// 测试完整工作流周期 - 合并多个测试
    #[test]
    #[cfg(feature = "csv")]
    fn test_complete_workflow_cycle() {
        let csv_file = "target/test_outputs/workflow_cycle.csv";
        fs::create_dir_all("target/test_outputs").ok();
        let _ = fs::remove_file(csv_file); // Pre-cleanup

        let config = create_test_config(csv_file);

        // Step 1: Validate config
        assert!(config.validate().is_ok());
        assert_eq!(config.sqllog.directory(), "target/test_outputs");
        assert!(!config.features.should_replace_sql_parameters());

        // Step 2: Create manager
        let manager = ExporterManager::from_config(&config);
        assert!(manager.is_ok(), "Failed to create ExporterManager");

        // Step 3: Manager lifecycle
        let mut manager = manager.unwrap();
        assert!(
            manager.initialize().is_ok(),
            "Failed to initialize ExporterManager"
        );
        assert!(
            manager.finalize().is_ok(),
            "Failed to finalize ExporterManager"
        );

        // Clean up
        let _ = fs::remove_file(csv_file);
    }
}

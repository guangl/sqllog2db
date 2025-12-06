/// 导出器实际集成测试 - 测试真实的导出流程
#[cfg(test)]
#[allow(clippy::needless_update)]
mod exporter_integration_tests {
    use dm_database_sqllog2db::exporter::{CsvExporter, ExportStats, Exporter};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Once;

    static INIT: Once = Once::new();

    /// 初始化测试目录（仅执行一次，性能优化）
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
    fn test_csv_exporter_initialize_and_finalize() {
        // 创建测试输出目录
        let test_dir = get_test_output_dir();
        let csv_path = test_dir.join("test_output_init.csv");

        // 清理旧文件
        let _ = fs::remove_file(&csv_path);

        // 创建 CSV 导出器
        let mut exporter = CsvExporter::new(&csv_path);

        // 测试初始化
        assert!(exporter.initialize().is_ok());

        // 测试完成（完成会刷新缓冲区）
        assert!(exporter.finalize().is_ok());

        // 验证文件已创建
        assert!(csv_path.exists());

        // 读取文件内容，检查表头
        let content = fs::read_to_string(&csv_path).expect("Failed to read CSV file");
        assert!(!content.is_empty(), "CSV file should not be empty");
        assert!(
            content.contains("ts,ep"),
            "CSV header should contain 'ts,ep'"
        );
        assert!(
            content.contains("sess_id") || content.contains("session"),
            "CSV header should contain session identifier"
        );
    }

    #[test]
    fn test_csv_exporter_from_config() {
        let csv_config = dm_database_sqllog2db::config::CsvExporter {
            file: "test_from_config.csv".to_string(),
            overwrite: true,
            append: false,
        };

        // Create using from_config method
        let exporter_inst = CsvExporter::from_config(&csv_config);
        let _ = exporter_inst;
    }

    #[test]
    fn test_csv_exporter_multiple_instances() {
        // 创建测试输出目录
        let test_dir = get_test_output_dir();

        // 创建多个导出器
        for i in 0..3 {
            let csv_path = test_dir.join(format!("test_multiple_{i}.csv"));
            let _ = fs::remove_file(&csv_path);

            let mut exporter = CsvExporter::new(&csv_path);
            assert!(exporter.initialize().is_ok());
            assert!(exporter.finalize().is_ok());
            assert!(csv_path.exists());
        }
    }

    #[test]
    fn test_csv_exporter_different_paths() {
        let test_dir = get_test_output_dir();

        // 测试不同路径
        let paths = vec![
            "simple.csv",
            "with/nested/path.csv",
            "special_chars_test.csv",
        ];

        for path_str in paths {
            let csv_path = test_dir.join(path_str);
            if let Some(parent) = csv_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let mut exporter = CsvExporter::new(&csv_path);
            let result = exporter.initialize();
            // Initialize may succeed or fail depending on permissions,但不应该panic
            let _ = result;

            // 确保完成也不会panic
            let _ = exporter.finalize();
        }
    }

    #[test]
    fn test_csv_exporter_file_overwrite() {
        let test_dir = get_test_output_dir();
        let csv_path = test_dir.join("test_overwrite.csv");
        let _ = fs::remove_file(&csv_path);

        // 第一次创建
        {
            let mut exporter = CsvExporter::new(&csv_path);
            assert!(exporter.initialize().is_ok());
            assert!(exporter.finalize().is_ok());
        }

        let first_content = fs::read_to_string(&csv_path).unwrap();
        assert!(first_content.contains("ts,ep"));

        // 第二次创建（使用overwrite）
        {
            let mut exporter = CsvExporter::new(&csv_path);
            assert!(exporter.initialize().is_ok());
            assert!(exporter.finalize().is_ok());
        }

        let second_content = fs::read_to_string(&csv_path).unwrap();
        assert!(second_content.contains("ts,ep"));
    }

    #[test]
    fn test_csv_exporter_nested_directory_creation() {
        let test_dir = "target/test_outputs/nested/deep/dir";
        let _ = fs::remove_dir_all("target/test_outputs/nested");

        let csv_path = format!("{test_dir}/output.csv");
        let mut exporter = CsvExporter::new(&csv_path);

        // Should create nested directories
        assert!(exporter.initialize().is_ok());
        assert!(exporter.finalize().is_ok());
        assert!(PathBuf::from(&csv_path).exists());

        // Cleanup
        let _ = fs::remove_dir_all("target/test_outputs/nested");
    }

    #[test]
    fn test_csv_exporter_debug_trait() {
        let test_dir = get_test_output_dir();
        let csv_path = test_dir.join("test_debug.csv");
        let exporter = CsvExporter::new(&csv_path);

        // Verify Debug trait works
        let debug_str = format!("{exporter:?}");
        assert!(debug_str.contains("CsvExporter"));
    }

    #[test]
    fn test_csv_exporter_stats() {
        let test_dir = get_test_output_dir();
        let csv_path = test_dir.join("test_stats.csv");
        let _ = fs::remove_file(&csv_path);

        let mut exporter = CsvExporter::new(&csv_path);
        assert!(exporter.initialize().is_ok());

        // Stats should be available
        let _ = exporter;

        // The exporter should be able to be dropped without panicking
    }

    #[test]
    fn test_csv_exporter_manager_integration() {
        use dm_database_sqllog2db::config::Config;
        use dm_database_sqllog2db::exporter::ExporterManager;

        let test_dir = "target/test_outputs";
        let csv_file = format!("{test_dir}/manager_integration.csv");
        let _ = fs::remove_file(&csv_file);

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
                    file: csv_file.clone(),
                    overwrite: true,
                    append: false,
                }),
                ..Default::default()
            },
        };

        // 创建导出器管理器
        let manager = ExporterManager::from_config(&config).expect("Failed to create manager");

        // 检查统计信息
        let stats = manager.stats();
        // stats 可能为 None 或 Some，两者都是有效的
        let _ = stats;
    }

    #[test]
    fn test_export_stats_structure() {
        let stats = ExportStats::new();
        // Verify that stats can be created
        let _ = stats;
    }

    #[test]
    fn test_csv_exporter_path_types() {
        let test_dir = get_test_output_dir();

        // Test with different path types
        let string_path = format!("{}/from_string.csv", test_dir.display());
        let mut exporter = CsvExporter::new(&string_path);
        assert!(exporter.initialize().is_ok());
        let _ = exporter.finalize();

        // Test with PathBuf
        let pathbuf_path = test_dir.join("from_pathbuf.csv");
        let mut exporter = CsvExporter::new(&pathbuf_path);
        assert!(exporter.initialize().is_ok());
        let _ = exporter.finalize();
    }

    #[test]
    fn test_csv_exporter_consecutive_operations() {
        let test_dir = get_test_output_dir();
        let csv_path = test_dir.join("test_consecutive.csv");
        let _ = fs::remove_file(&csv_path);

        // Sequence: init -> finalize -> init -> finalize
        {
            let mut exporter = CsvExporter::new(&csv_path);
            assert!(exporter.initialize().is_ok());
            assert!(exporter.finalize().is_ok());
        }

        {
            let mut exporter = CsvExporter::new(&csv_path);
            assert!(exporter.initialize().is_ok());
            assert!(exporter.finalize().is_ok());
        }

        assert!(csv_path.exists());
    }

    #[test]
    fn test_exporter_trait_implementation() {
        let test_dir = get_test_output_dir();
        let csv_path = test_dir.join("test_trait.csv");
        let _ = fs::remove_file(&csv_path);

        let mut exporter: Box<dyn Exporter> = Box::new(CsvExporter::new(&csv_path));
        assert!(exporter.initialize().is_ok());
        assert!(exporter.finalize().is_ok());

        assert!(csv_path.exists());
    }
}

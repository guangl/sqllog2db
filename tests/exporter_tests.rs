/// 导出器和日志功能测试
#[cfg(test)]
mod exporter_and_logging_tests {
    use dm_database_sqllog2db::config::{
        CsvExporter, ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig, SqllogConfig,
    };
    use dm_database_sqllog2db::constants::LOG_LEVELS;

    #[test]
    fn test_csv_exporter_creation() {
        // 测试 CSV 导出器创建
        let exporter = CsvExporter {
            file: "output.csv".to_string(),
            overwrite: false,
            append: false,
        };

        assert_eq!(exporter.file, "output.csv");
        assert!(!exporter.overwrite);
        assert!(!exporter.append);
    }

    #[test]
    fn test_csv_exporter_overwrite() {
        // 测试 CSV 导出器覆盖模式
        let exporter = CsvExporter {
            file: "output.csv".to_string(),
            overwrite: true,
            append: false,
        };

        assert!(exporter.overwrite);
        assert!(!exporter.append);
    }

    #[test]
    fn test_csv_exporter_append() {
        // 测试 CSV 导出器追加模式
        let exporter = CsvExporter {
            file: "output.csv".to_string(),
            overwrite: false,
            append: true,
        };

        assert!(!exporter.overwrite);
        assert!(exporter.append);
    }

    #[test]
    fn test_log_levels_constant() {
        // 测试日志级别常量
        assert!(!LOG_LEVELS.is_empty());

        // 验证包含标准的日志级别
        let level_strings: Vec<&str> = LOG_LEVELS.to_vec();
        assert!(level_strings.len() >= 4);
    }

    #[test]
    fn test_log_levels_case_insensitive() {
        // 测试日志级别大小写不敏感
        let levels = ["debug", "Debug", "DEBUG", "info", "Info", "INFO"];

        for level in levels {
            let found = LOG_LEVELS.iter().any(|&l| l.eq_ignore_ascii_case(level));
            assert!(found, "Level {level} should be found (case-insensitive)");
        }
    }

    #[test]
    fn test_full_config_structure() {
        // 测试完整配置结构
        let sqllog = SqllogConfig {
            directory: "sqllogs".to_string(),
        };

        let error = ErrorConfig {
            file: "export/errors.log".to_string(),
        };

        let logging = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };

        let features = FeaturesConfig {
            replace_parameters: None,
        };

        let exporter = ExporterConfig {
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

        // 验证所有配置组件已正确创建
        assert_eq!(sqllog.directory(), "sqllogs");
        assert_eq!(error.file(), "export/errors.log");
        assert_eq!(logging.file(), "logs/app.log");
        assert!(!features.should_replace_sql_parameters());
        assert!(!exporter.has_exporters());
    }

    #[test]
    fn test_csv_exporter_default() {
        // 测试 CSV 导出器默认实现
        let exporter = CsvExporter {
            file: "test.csv".to_string(),
            overwrite: true,
            append: false,
        };

        // 验证导出器字段
        assert!(!exporter.file.is_empty());
    }

    #[test]
    fn test_multiple_exporters_together() {
        // 测试多个导出器配置一起使用
        let csv = CsvExporter {
            file: "output.csv".to_string(),
            overwrite: false,
            append: false,
        };

        #[cfg(feature = "csv")]
        {
            let exporter_config = ExporterConfig {
                #[cfg(feature = "csv")]
                csv: Some(csv.clone()),
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

            assert!(exporter_config.has_exporters());
            assert!(exporter_config.csv().is_some());
        }
    }

    #[test]
    fn test_logging_levels_enumeration() {
        // 测试所有日志级别都有效
        for &level in LOG_LEVELS {
            let config = LoggingConfig {
                file: "test.log".to_string(),
                level: level.to_string(),
                retention_days: 7,
            };

            assert!(config.validate().is_ok(), "Level {level} validation failed");
        }
    }

    #[test]
    fn test_config_validation_chain() {
        // 测试完整的配置验证链
        let sqllog = SqllogConfig {
            directory: "logs".to_string(),
        };

        let logging = LoggingConfig {
            file: "output.log".to_string(),
            level: "debug".to_string(),
            retention_days: 30,
        };

        // 验证每个配置组件
        assert!(sqllog.validate().is_ok());
        assert!(logging.validate().is_ok());
    }
}

/// CSV 导出器配置测试
#[cfg(test)]
mod csv_exporter_config_tests {
    use dm_database_sqllog2db::config::CsvExporter;

    #[test]
    fn test_csv_default_values() {
        // 测试 CSV 导出器默认值
        let exporter = CsvExporter {
            file: "output.csv".to_string(),
            overwrite: false,
            append: false,
        };

        assert_eq!(exporter.file, "output.csv");
        assert!(!exporter.overwrite);
        assert!(!exporter.append);
    }

    #[test]
    fn test_csv_various_paths() {
        // 测试各种文件路径
        let paths = vec![
            "output.csv",
            "export/output.csv",
            "./data/output.csv",
            "C:\\export\\output.csv",
        ];

        for path in paths {
            let exporter = CsvExporter {
                file: path.to_string(),
                overwrite: false,
                append: false,
            };

            assert_eq!(exporter.file, path);
        }
    }

    #[test]
    fn test_csv_mode_combinations() {
        // 测试各种模式组合
        let combinations = vec![
            (false, false, "normal"),
            (true, false, "overwrite"),
            (false, true, "append"),
            (true, true, "both_flags"), // 实际使用中应该选择一个
        ];

        for (overwrite, append, _desc) in combinations {
            let exporter = CsvExporter {
                file: "test.csv".to_string(),
                overwrite,
                append,
            };

            assert_eq!(exporter.overwrite, overwrite);
            assert_eq!(exporter.append, append);
        }
    }

    #[test]
    fn test_csv_exporter_debug_format() {
        // 测试 CSV 导出器调试输出
        let exporter = CsvExporter {
            file: "output.csv".to_string(),
            overwrite: true,
            append: false,
        };

        let debug_str = format!("{exporter:?}");
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("CsvExporter"));
        assert!(debug_str.contains("output.csv"));
    }
}

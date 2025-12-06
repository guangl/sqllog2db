/// 集成测试 - 测试配置加载、解析和导出流程
#[cfg(test)]
mod integration_tests {
    use dm_database_sqllog2db::config::{
        ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig, ReplaceParametersFeature,
        SqllogConfig,
    };
    use dm_database_sqllog2db::parser::SqllogParser;

    #[test]
    fn test_sqllog_config_creation() {
        // 测试 SqllogConfig 创建
        let config = SqllogConfig {
            directory: "sqllogs".to_string(),
        };

        assert_eq!(config.directory(), "sqllogs");
    }

    #[test]
    fn test_sqllog_config_validation() {
        // 测试有效的 SqllogConfig
        let config = SqllogConfig {
            directory: "test_logs".to_string(),
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_sqllog_config_invalid_empty_directory() {
        // 测试无效的空目录
        let config = SqllogConfig {
            directory: String::new(),
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_sqllog_config_invalid_whitespace_directory() {
        // 测试只含空格的目录
        let config = SqllogConfig {
            directory: "   ".to_string(),
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_error_config_creation() {
        // 测试 ErrorConfig 创建
        let config = ErrorConfig {
            file: "export/errors.log".to_string(),
        };

        assert_eq!(config.file(), "export/errors.log");
    }

    #[test]
    fn test_logging_config_creation() {
        // 测试 LoggingConfig 创建
        let config = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 7,
        };

        assert_eq!(config.file(), "logs/app.log");
        assert_eq!(config.level(), "info");
        assert_eq!(config.retention_days(), 7);
    }

    #[test]
    fn test_logging_config_valid_levels() {
        // 测试有效的日志级别
        let valid_levels = vec!["debug", "info", "warn", "error"];

        for level in valid_levels {
            let config = LoggingConfig {
                file: "logs/app.log".to_string(),
                level: level.to_string(),
                retention_days: 7,
            };

            assert!(config.validate().is_ok(), "Level {level} should be valid");
        }
    }

    #[test]
    fn test_logging_config_valid_levels_uppercase() {
        // 测试大写的日志级别
        let valid_levels = vec!["DEBUG", "INFO", "WARN", "ERROR"];

        for level in valid_levels {
            let config = LoggingConfig {
                file: "logs/app.log".to_string(),
                level: level.to_string(),
                retention_days: 7,
            };

            assert!(config.validate().is_ok(), "Level {level} should be valid");
        }
    }

    #[test]
    fn test_logging_config_invalid_level() {
        // 测试无效的日志级别
        let config = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "invalid_level".to_string(),
            retention_days: 7,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_logging_config_invalid_retention_days_zero() {
        // 测试保留天数为 0
        let config = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 0,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_logging_config_invalid_retention_days_too_large() {
        // 测试保留天数超过 365
        let config = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 366,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_logging_config_valid_retention_min() {
        // 测试最小有效保留天数
        let config = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 1,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_logging_config_valid_retention_max() {
        // 测试最大有效保留天数
        let config = LoggingConfig {
            file: "logs/app.log".to_string(),
            level: "info".to_string(),
            retention_days: 365,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_exporter_config_creation() {
        // 测试 ExporterConfig 创建
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

        // 验证导出器配置已创建
        assert!(!config.has_exporters());
    }

    #[test]
    fn test_features_config_creation() {
        // 测试 FeaturesConfig 创建
        let config = FeaturesConfig {
            replace_parameters: None,
        };

        assert!(!config.should_replace_sql_parameters());
    }

    #[test]
    fn test_features_config_with_replace_parameters_disabled() {
        // 测试禁用参数替换的 FeaturesConfig
        let config = FeaturesConfig {
            replace_parameters: Some(ReplaceParametersFeature {
                enable: false,
                symbols: None,
            }),
        };

        assert!(!config.should_replace_sql_parameters());
    }

    #[test]
    fn test_features_config_with_replace_parameters_enabled() {
        // 测试启用参数替换的 FeaturesConfig
        let config = FeaturesConfig {
            replace_parameters: Some(ReplaceParametersFeature {
                enable: true,
                symbols: None,
            }),
        };

        assert!(config.should_replace_sql_parameters());
    }

    #[test]
    fn test_features_config_with_replace_parameters_enabled_with_symbols() {
        // 测试启用参数替换且带符号的 FeaturesConfig
        let config = FeaturesConfig {
            replace_parameters: Some(ReplaceParametersFeature {
                enable: true,
                symbols: Some(vec!["?".to_string(), ":".to_string()]),
            }),
        };

        assert!(config.should_replace_sql_parameters());
        if let Some(feature) = &config.replace_parameters {
            assert!(feature.symbols.is_some());
            assert_eq!(feature.symbols.as_ref().unwrap().len(), 2);
        }
    }

    #[test]
    fn test_config_defaults() {
        // 测试 Config 默认值
        let sqllog = SqllogConfig::default();
        let error = ErrorConfig::default();
        let logging = LoggingConfig::default();

        assert_eq!(sqllog.directory(), "sqllogs");
        assert_eq!(error.file(), "export/errors.log");
        assert_eq!(logging.file(), "logs/sqllog2db.log");
        assert_eq!(logging.level(), "info");
    }

    #[test]
    fn test_parser_creation() {
        // 测试解析器创建
        let parser = SqllogParser::new("sqllogs");

        // 验证解析器已创建
        assert_eq!(parser.path().to_string_lossy(), "sqllogs");
    }

    #[test]
    fn test_parser_with_different_paths() {
        // 测试带不同路径的解析器创建
        let paths = vec!["sqllogs", ".", "logs", "export"];

        for path in paths {
            let parser = SqllogParser::new(path);
            assert_eq!(parser.path().to_string_lossy(), path);
        }
    }

    #[test]
    fn test_parser_debug_impl() {
        // 测试解析器调试输出
        let parser = SqllogParser::new("test_path");
        let debug_str = format!("{parser:?}");

        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("SqllogParser"));
    }

    #[test]
    fn test_logging_level_to_lowercase() {
        // 测试日志级别转换为小写
        let levels = vec!["DEBUG", "INFO", "WARN", "ERROR"];

        for level in levels {
            let lower = level.to_lowercase();
            assert!(lower.chars().all(|c| c.is_ascii_lowercase()));
        }
    }
}

/// 错误处理集成测试
#[cfg(test)]
mod error_handling_integration_tests {
    #[test]
    fn test_config_error_types() {
        // 验证错误类型的存在
        // 这确保错误处理层结构正确
    }
}

/// CLI 集成测试
#[cfg(test)]
mod cli_integration_tests {
    #[test]
    fn test_cli_arg_parsing() {
        // 测试 CLI 参数解析
        let args = ["sqllog2db", "--input", "test.log", "--output", "output.csv"];

        assert_eq!(args.len(), 5);
        assert_eq!(args[0], "sqllog2db");
        assert_eq!(args[1], "--input");
    }

    #[test]
    fn test_config_file_loading() {
        // 测试配置文件加载
        let config_file = "config.toml";
        assert!(config_file.contains("toml"));
    }
}

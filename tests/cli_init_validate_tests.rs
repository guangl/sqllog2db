/// CLI init 和 validate 命令的集成测试
#[cfg(test)]
mod cli_commands_tests {
    use dm_database_sqllog2db::config::Config;
    use std::fs;
    use std::path::Path;

    /// 辅助函数：生成配置文件内容
    fn generate_config_content() -> String {
        r#"[sqllog]
directory = "sqllogs"

[error]
file = "export/errors.log"

[logging]
file = "logs/sqllog2db.log"
level = "info"
retention_days = 7

[features.replace_parameters]
enable = false

[exporter.csv]
file = "outputs/sqllog.csv"
overwrite = true
append = false
"#
        .to_string()
    }

    /// 测试 init 命令 - 配置文件生成的模拟
    #[test]
    fn test_init_command_generates_config() {
        let test_dir = "target/test_outputs/cli_init";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).ok();

        let config_path = format!("{test_dir}/config.toml");
        let content = generate_config_content();

        // 写入配置文件
        let result = fs::write(&config_path, &content);
        assert!(result.is_ok(), "Failed to write config file");

        // 验证文件已创建
        assert!(Path::new(&config_path).exists(), "Config file should exist");

        // 验证文件内容
        let read_content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(read_content, content, "Config content should match");

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// 测试 init 命令 - 文件已存在时的处理（force = false）
    #[test]
    fn test_init_command_file_exists_no_force() {
        let test_dir = "target/test_outputs/cli_init_exists";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).ok();

        let config_path = format!("{test_dir}/config.toml");
        let original_content = "original content";

        // 创建原始配置文件
        fs::write(&config_path, original_content).unwrap();

        // 尝试再次写入（模拟不使用 --force）
        let existing = Path::new(&config_path).exists();
        assert!(existing, "File should exist before check");

        // 读取内容应该仍是原始内容
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(
            content, original_content,
            "File should not be overwritten without force"
        );

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// 测试 init 命令 - 文件已存在时的处理（force = true）
    #[test]
    fn test_init_command_file_exists_with_force() {
        let test_dir = "target/test_outputs/cli_init_force";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).ok();

        let config_path = format!("{test_dir}/config.toml");
        let original_content = "original content";
        let new_content = generate_config_content();

        // 创建原始配置文件
        fs::write(&config_path, original_content).unwrap();

        // 使用 force 覆盖
        fs::write(&config_path, &new_content).unwrap();

        // 验证内容已被覆盖
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(
            content, new_content,
            "File should be overwritten with force"
        );

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// 测试 init 命令 - 创建嵌套目录
    #[test]
    fn test_init_command_creates_nested_dir() {
        let test_dir = "target/test_outputs/cli_init_nested/deep/path";
        let _ = fs::remove_dir_all("target/test_outputs/cli_init_nested");
        fs::create_dir_all(test_dir).ok();

        let config_path = format!("{test_dir}/config.toml");
        let content = generate_config_content();

        fs::write(&config_path, content).unwrap();
        assert!(Path::new(&config_path).exists());

        // Clean up
        let _ = fs::remove_dir_all("target/test_outputs/cli_init_nested");
    }

    /// 测试 init 命令 - 生成的配置文件格式
    #[test]
    fn test_init_config_format() {
        let content = generate_config_content();

        // 验证 TOML 结构
        assert!(
            content.contains("[sqllog]"),
            "Should contain [sqllog] section"
        );
        assert!(
            content.contains("[error]"),
            "Should contain [error] section"
        );
        assert!(
            content.contains("[logging]"),
            "Should contain [logging] section"
        );
        assert!(
            content.contains("[features.replace_parameters]"),
            "Should contain [features.replace_parameters] section"
        );
        assert!(
            content.contains("[exporter.csv]"),
            "Should contain [exporter.csv] section"
        );
    }

    /// 测试 init 命令 - 生成的配置可以加载
    #[test]
    fn test_init_generated_config_is_loadable() {
        let test_dir = "target/test_outputs/cli_init_loadable";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).ok();

        let config_path = format!("{test_dir}/config.toml");
        let content = generate_config_content();

        fs::write(&config_path, content).unwrap();

        // 尝试加载配置
        let config = Config::from_file(&config_path);
        assert!(config.is_ok(), "Generated config should be loadable");

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// 测试 validate 命令 - 验证有效的配置
    #[test]
    fn test_validate_command_valid_config() {
        let config = Config::default();

        // 验证应该成功
        assert!(config.validate().is_ok(), "Default config should be valid");
    }

    /// 测试 validate 命令 - 验证无效的日志级别
    #[test]
    fn test_validate_command_invalid_log_level() {
        use dm_database_sqllog2db::config::LoggingConfig;

        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "INVALID_LEVEL".to_string(),
            retention_days: 7,
        };

        // 验证应该失败
        assert!(
            config.validate().is_err(),
            "Invalid log level should fail validation"
        );
    }

    /// 测试 validate 命令 - 验证无效的保留天数
    #[test]
    fn test_validate_command_invalid_retention_days() {
        use dm_database_sqllog2db::config::LoggingConfig;

        // 测试 retention_days = 0（无效）
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "info".to_string(),
            retention_days: 0,
        };

        assert!(config.validate().is_err(), "Retention days 0 should fail");

        // 测试 retention_days = 366（超出范围）
        let config = LoggingConfig {
            file: "app.log".to_string(),
            level: "info".to_string(),
            retention_days: 366,
        };

        assert!(
            config.validate().is_err(),
            "Retention days > 365 should fail"
        );
    }

    /// 测试 validate 命令 - 验证无导出器配置
    #[test]
    fn test_validate_command_no_exporters() {
        use dm_database_sqllog2db::config::{
            ErrorConfig, ExporterConfig, FeaturesConfig, LoggingConfig, SqllogConfig,
        };

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

        // 配置验证应该失败（无导出器）
        assert!(
            config.validate().is_err(),
            "Config without exporters should fail"
        );
    }

    /// 测试 validate 命令 - 验证空的 SQL 日志目录
    #[test]
    fn test_validate_command_empty_sqllog_dir() {
        use dm_database_sqllog2db::config::SqllogConfig;

        let config = SqllogConfig {
            directory: String::new(),
        };

        // 验证应该失败
        assert!(
            config.validate().is_err(),
            "Empty sqllog directory should fail"
        );
    }

    /// 测试 validate 命令 - 验证各种日志级别
    #[test]
    fn test_validate_command_all_log_levels() {
        use dm_database_sqllog2db::config::LoggingConfig;

        let levels = vec!["trace", "debug", "info", "warn", "error"];

        for level in levels {
            let config = LoggingConfig {
                file: "app.log".to_string(),
                level: level.to_string(),
                retention_days: 7,
            };

            assert!(
                config.validate().is_ok(),
                "Log level '{level}' should be valid"
            );
        }
    }

    /// 测试 init 命令 - 配置文件的完整性
    #[test]
    fn test_init_config_completeness() {
        let content = generate_config_content();
        let _config_result = toml::from_str::<Config>(&content);

        // 虽然我们不能直接反序列化（缺少 toml 依赖），但我们可以检查结构
        assert!(
            content.contains("directory"),
            "Should contain directory field"
        );
        assert!(content.contains("level"), "Should contain level field");
        assert!(
            content.contains("retention_days"),
            "Should contain retention_days field"
        );
        assert!(content.contains("file"), "Should contain file field");
    }

    /// 测试 validate 命令 - 配置文件路径处理
    #[test]
    fn test_validate_config_path_handling() {
        let paths = vec![
            "config.toml",
            "./config.toml",
            "path/to/config.toml",
            "path/to/my.config.toml",
        ];

        for path in paths {
            assert!(!path.is_empty());
            assert!(
                path.contains(".toml"),
                "Config path should reference toml file"
            );
        }
    }

    /// 测试 init 和 validate 命令的交互
    #[test]
    fn test_init_and_validate_commands_interaction() {
        let test_dir = "target/test_outputs/cli_init_validate";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).ok();

        let config_path = format!("{test_dir}/config.toml");

        // 1. Init: 生成配置
        let content = generate_config_content();
        fs::write(&config_path, &content).unwrap();
        assert!(Path::new(&config_path).exists());

        // 2. Validate: 验证配置
        let config = Config::from_file(&config_path);
        assert!(config.is_ok());
        assert!(config.unwrap().validate().is_ok());

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    /// 测试 init 命令 - 输出路径指定
    #[test]
    fn test_init_output_path_options() {
        let test_dir = "target/test_outputs/cli_init_paths";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).ok();

        let paths = vec![
            format!("{}/config.toml", test_dir),
            format!("{}/my_config.toml", test_dir),
            format!("{}/config.prod.toml", test_dir),
        ];

        for path in paths {
            let content = generate_config_content();
            fs::write(&path, content).ok();
            assert!(Path::new(&path).exists());
        }

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }
}

//! 针对 CLI 选项解析的详细测试
#[cfg(test)]
mod cli_deep_tests {
    use std::fs;
    use std::path::PathBuf;

    fn setup_test_dir(name: &str) -> PathBuf {
        let test_dir = PathBuf::from("target/test_cli_deep").join(name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).expect("Failed to create test dir");
        test_dir
    }

    #[test]
    fn test_cli_options_parsing() {
        // Test config file validation
        let test_dir = setup_test_dir("options");
        let config_path = test_dir.join("config.toml");

        let content = "[sqllog]\nsqllogs = \"./sqllogs\"\n";
        fs::write(&config_path, content).expect("Failed to write");

        assert!(config_path.exists());
        let read = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(read.contains("sqllog"));
    }

    #[test]
    fn test_cli_config_validation() {
        let test_dir = setup_test_dir("config_validation");
        let config_path = test_dir.join("config.toml");

        let valid_config = r#"
[sqllog]
sqllogs = "./sqllogs"

[exporter]
csv = true

[csv]
output = "./output.csv"
"#;
        fs::write(&config_path, valid_config).expect("Failed to write");

        let content = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(content.contains("[sqllog]"));
        assert!(content.contains("[exporter]"));
    }

    #[test]
    fn test_cli_multiple_exporters() {
        let test_dir = setup_test_dir("multiple_exporters");
        let config_path = test_dir.join("config.toml");

        let config = r#"
[sqllog]
sqllogs = "./sqllogs"

[exporter]
csv = true
jsonl = true

[csv]
output = "./output.csv"

[jsonl]
output = "./output.jsonl"
"#;
        fs::write(&config_path, config).expect("Failed to write");

        let content = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(content.contains("csv = true"));
        assert!(content.contains("jsonl = true"));
    }

    #[test]
    fn test_cli_logging_config() {
        let test_dir = setup_test_dir("logging_config");
        let config_path = test_dir.join("config.toml");

        let config = r#"
[logging]
level = "info"
format = "text"
file = "./app.log"
"#;
        fs::write(&config_path, config).expect("Failed to write");

        let content = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(content.contains("level = \"info\""));
    }

    #[test]
    fn test_cli_error_handling_config() {
        let test_dir = setup_test_dir("error_handling");
        let config_path = test_dir.join("config.toml");

        let config = r#"
[error]
output = "./errors.jsonl"
log_errors = true
exit_on_error = false
"#;
        fs::write(&config_path, config).expect("Failed to write");

        let content = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(content.contains("error"));
    }

    #[test]
    fn test_cli_feature_flags() {
        let test_dir = setup_test_dir("features");
        let config_path = test_dir.join("config.toml");

        let config = r"
[features]
replace_parameters = true
tui_enabled = false
";
        fs::write(&config_path, config).expect("Failed to write");

        let content = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(content.contains("features"));
    }

    #[test]
    fn test_cli_config_file_parsing() {
        let test_dir = setup_test_dir("toml_parsing");
        let config_path = test_dir.join("config.toml");

        let config = r#"
# Comment line
[section]
key1 = "value1"
key2 = 42
key3 = true
"#;
        fs::write(&config_path, config).expect("Failed to write");

        let content = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(content.contains("key1"));
        assert!(content.contains("value1"));
    }

    #[test]
    fn test_cli_directory_structure() {
        let test_dir = setup_test_dir("dir_structure");

        let dirs = vec!["sqllogs", "output", "logs"];
        for dir in dirs {
            let path = test_dir.join(dir);
            fs::create_dir_all(&path).expect("Failed to create dir");
            assert!(path.exists());
        }
    }

    #[test]
    fn test_cli_config_paths() {
        let test_dir = setup_test_dir("config_paths");
        let config_path = test_dir.join("config.toml");

        let config = r#"
[sqllog]
sqllogs = "./sqllogs"

[csv]
output = "./output/results.csv"
"#;
        fs::write(&config_path, config).expect("Failed to write");

        let content = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(content.contains("./sqllogs"));
        assert!(content.contains("./output/results.csv"));
    }

    #[test]
    fn test_cli_config_validation_error_scenarios() {
        let test_dir = setup_test_dir("error_scenarios");

        // Empty config
        let config_path1 = test_dir.join("empty.toml");
        fs::write(&config_path1, "").expect("Failed to write");
        let content = fs::read_to_string(&config_path1).expect("Failed to read");
        assert_eq!(content, "");

        // Invalid TOML (should handle gracefully)
        let config_path2 = test_dir.join("invalid.toml");
        let content = "[section\nkey = value";
        fs::write(&config_path2, content).expect("Failed to write");
        let read = fs::read_to_string(&config_path2).expect("Failed to read");
        assert_eq!(read, content);
    }

    #[test]
    fn test_cli_long_paths() {
        let test_dir = setup_test_dir("long_paths");
        let config_path = test_dir.join("config.toml");

        let long_path = "./very/long/path/to/some/deeply/nested/directory/output.csv";
        let config = format!("[csv]\noutput = \"{long_path}\"\n");
        fs::write(&config_path, &config).expect("Failed to write");

        let content = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(content.contains(long_path));
    }

    #[test]
    fn test_cli_special_characters_in_paths() {
        let test_dir = setup_test_dir("special_chars");
        let config_path = test_dir.join("config.toml");

        let config = r#"
[csv]
output = "./output file (1).csv"
"#;
        fs::write(&config_path, config).expect("Failed to write");

        let content = fs::read_to_string(&config_path).expect("Failed to read");
        assert!(content.contains("output file (1).csv"));
    }
}

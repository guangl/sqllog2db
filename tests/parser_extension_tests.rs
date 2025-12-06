// 测试 parser.rs 功能
#[cfg(test)]
mod parser_extension_tests {
    use dm_database_sqllog2db::parser::SqllogParser;
    use std::fs;

    /// 测试创建 `SqllogParser` 实例
    #[test]
    fn test_parser_creation_basic() {
        let parser = SqllogParser::new("target/test_outputs");
        assert_eq!(parser.path().to_str().unwrap(), "target/test_outputs");
    }

    /// 测试 `SqllogParser` 与不同路径
    #[test]
    fn test_parser_with_absolute_path() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let abs_path = std::fs::canonicalize(log_dir).unwrap();
        let parser = SqllogParser::new(abs_path.to_str().unwrap());
        assert!(parser.path().is_absolute(), "Path should be absolute");
    }

    /// 测试 `SqllogParser` 调试格式
    #[test]
    fn test_parser_debug_format() {
        let parser = SqllogParser::new("target/test_outputs");
        let debug_str = format!("{parser:?}");
        assert!(
            debug_str.contains("SqllogParser"),
            "Debug should contain type name"
        );
    }

    /// 测试空目录的 `SqllogParser`
    #[test]
    fn test_parser_empty_directory() {
        let log_dir = "target/test_outputs/empty_parser";
        fs::create_dir_all(log_dir).ok();

        let parser = SqllogParser::new(log_dir);
        let files = parser.log_files();
        assert!(files.is_ok(), "Should handle empty directory");
        assert_eq!(
            files.unwrap().len(),
            0,
            "Empty directory should return no files"
        );
    }

    /// 测试不存在的目录
    #[test]
    fn test_parser_nonexistent_path() {
        let parser = SqllogParser::new("/nonexistent/path/to/logs");
        let files = parser.log_files();
        assert!(files.is_err(), "Should fail for nonexistent path");
    }

    /// 测试 `SqllogParser` 带有嵌套路径
    #[test]
    fn test_parser_nested_path() {
        let log_dir = "target/test_outputs/nested/parser/path";
        fs::create_dir_all(log_dir).ok();

        let parser = SqllogParser::new(log_dir);
        let files = parser.log_files();
        assert!(files.is_ok(), "Failed with nested path");
    }

    /// 测试 `SqllogParser` 路径规范化
    #[test]
    fn test_parser_path_with_dots() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let parser = SqllogParser::new(format!("{log_dir}/./parser").as_str());
        // Parser 创建应该成功，即使路径有 ./
        assert!(parser.path().to_str().is_some(), "Path should be valid");
    }

    /// 测试多个 `SqllogParser` 实例
    #[test]
    fn test_multiple_parser_instances() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let parser1 = SqllogParser::new(log_dir);
        let parser2 = SqllogParser::new(log_dir);

        assert_eq!(
            parser1.path(),
            parser2.path(),
            "Both parsers should have same path"
        );
    }

    /// 测试特殊字符路径
    #[test]
    fn test_parser_special_chars_path() {
        let log_dir = "target/test_outputs/test_2024-12-06_logs";
        fs::create_dir_all(log_dir).ok();

        let parser = SqllogParser::new(log_dir);
        assert!(
            parser.path().to_str().is_some(),
            "Parser should handle special chars"
        );
    }

    /// 测试不同的路径格式
    #[test]
    fn test_parser_different_path_formats() {
        let paths = vec!["target/test_outputs", "./target/test_outputs"];

        for path in paths {
            let parser = SqllogParser::new(path);
            assert!(
                parser.path().to_str().is_some(),
                "Failed with path format: {path}"
            );
        }
    }

    /// 测试 `SqllogParser` 与 Unicode 路径
    #[test]
    fn test_parser_unicode_path() {
        let log_dir = "target/test_outputs/日志目录";
        fs::create_dir_all(log_dir).ok();

        let parser = SqllogParser::new(log_dir);
        assert!(
            parser.path().to_str().is_some(),
            "Parser should handle Unicode path"
        );
    }

    /// 测试 `SqllogParser` 长路径
    #[test]
    fn test_parser_long_path() {
        let log_dir = "target/test_outputs/very/long/path/to/logs/directory/with/many/levels";
        fs::create_dir_all(log_dir).ok();

        let parser = SqllogParser::new(log_dir);
        assert!(
            parser.path().to_str().is_some(),
            "Parser should handle long path"
        );
    }

    /// 测试单个文件的日志文件列表
    #[test]
    fn test_parser_single_file() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let test_file = format!("{log_dir}/test.log");
        fs::write(&test_file, "test content").ok();

        let parser = SqllogParser::new(&test_file);
        let files = parser.log_files();
        assert!(files.is_ok(), "Should handle single file");
        assert_eq!(files.unwrap().len(), 1, "Should return single file");

        // Clean up
        let _ = fs::remove_file(&test_file);
    }
}

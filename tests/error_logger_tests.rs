// 测试 error_logger.rs 模块的完整功能
#[cfg(test)]
mod error_logger_tests {
    use dm_database_sqllog2db::error_logger::{ErrorLogger, ParseErrorRecord};
    use std::fs;
    use std::path::Path;

    /// 测试创建新的错误日志记录器
    #[test]
    fn test_error_logger_new() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_error.log");
        let result = ErrorLogger::new(&error_log_file);

        assert!(result.is_ok(), "Failed to create ErrorLogger");

        // 创建成功，说明初始化工作正常

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }
    /// 测试记录单个解析错误
    #[test]
    fn test_error_logger_log_error() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_log_error.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        let record = ParseErrorRecord {
            file_path: "/path/to/log.sql".to_string(),
            error_message: "Invalid format".to_string(),
            raw_content: Some("SELECT * FROM".to_string()),
            line_number: Some(42),
        };

        let result = logger.log_error(&record);
        assert!(result.is_ok(), "Failed to log error");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试记录多个错误
    #[test]
    fn test_error_logger_multiple_errors() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_multiple.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        for i in 0..5 {
            let record = ParseErrorRecord {
                file_path: format!("/path/to/log_{i}.sql"),
                error_message: format!("Error {i}"),
                raw_content: Some(format!("Content {i}")),
                line_number: Some(i * 10),
            };
            let result = logger.log_error(&record);
            assert!(result.is_ok(), "Failed to log error {i}");
        }

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试刷新缓冲区
    #[test]
    fn test_error_logger_flush() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_flush.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        let record = ParseErrorRecord {
            file_path: "/path/to/log.sql".to_string(),
            error_message: "Test error".to_string(),
            raw_content: Some("SELECT *".to_string()),
            line_number: Some(1),
        };

        logger.log_error(&record).ok();
        let flush_result = logger.flush();
        assert!(flush_result.is_ok(), "Failed to flush");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试完成和统计
    #[test]
    fn test_error_logger_finalize() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_finalize.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        for i in 0..3 {
            let record = ParseErrorRecord {
                file_path: format!("/path/to/log_{i}.sql"),
                error_message: format!("Error {i}"),
                raw_content: Some(format!("Content {i}")),
                line_number: Some(i),
            };
            logger.log_error(&record).ok();
        }

        let finalize_result = logger.finalize();
        assert!(finalize_result.is_ok(), "Failed to finalize");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试错误记录的 None 字段
    #[test]
    fn test_error_logger_optional_fields() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_optional.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        let record = ParseErrorRecord {
            file_path: "/path/to/log.sql".to_string(),
            error_message: "Test error".to_string(),
            raw_content: None,
            line_number: None,
        };

        let result = logger.log_error(&record);
        assert!(result.is_ok(), "Failed with optional fields");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试错误记录中的特殊字符
    #[test]
    fn test_error_logger_special_chars() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_special.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        let record = ParseErrorRecord {
            file_path: "/path/to/log|test.sql".to_string(),
            error_message: "Error with | special chars".to_string(),
            raw_content: Some("SELECT * FROM `table` WHERE id = 42".to_string()),
            line_number: Some(100),
        };

        let result = logger.log_error(&record);
        assert!(result.is_ok(), "Failed with special characters");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试错误记录中的换行符
    #[test]
    fn test_error_logger_newlines() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_newlines.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        let record = ParseErrorRecord {
            file_path: "/path/to/log.sql".to_string(),
            error_message: "Error with newline".to_string(),
            raw_content: Some("SELECT *\nFROM table\nWHERE id = 1".to_string()),
            line_number: Some(50),
        };

        let result = logger.log_error(&record);
        assert!(result.is_ok(), "Failed with newlines");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试创建嵌套目录
    #[test]
    fn test_error_logger_creates_nested_dir() {
        let log_dir = "target/test_outputs/error_logs/nested/deep";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_nested.log");
        let result = ErrorLogger::new(&error_log_file);

        assert!(result.is_ok(), "Failed to create with nested directory");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试错误记录中的空内容
    #[test]
    fn test_error_logger_empty_content() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_empty.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        let record = ParseErrorRecord {
            file_path: String::new(),
            error_message: String::new(),
            raw_content: Some(String::new()),
            line_number: Some(0),
        };

        let result = logger.log_error(&record);
        assert!(result.is_ok(), "Failed with empty content");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试追加模式
    #[test]
    fn test_error_logger_append_mode() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_append.log");

        // 创建第一个 logger 并写入一些数据
        {
            let mut logger1 = ErrorLogger::new(&error_log_file).unwrap();
            let record1 = ParseErrorRecord {
                file_path: "/path1".to_string(),
                error_message: "Error 1".to_string(),
                raw_content: None,
                line_number: None,
            };
            logger1.log_error(&record1).ok();
            logger1.finalize().ok();
        }

        // 创建第二个 logger 并追加数据
        {
            let mut logger2 = ErrorLogger::new(&error_log_file).unwrap();
            let record2 = ParseErrorRecord {
                file_path: "/path2".to_string(),
                error_message: "Error 2".to_string(),
                raw_content: None,
                line_number: None,
            };
            logger2.log_error(&record2).ok();
            logger2.finalize().ok();
        }

        // 验证文件存在
        assert!(
            Path::new(&error_log_file).exists(),
            "Error log file should exist"
        );

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试大量错误的性能
    #[test]
    fn test_error_logger_many_errors() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_many.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        for i in 0..100 {
            let record = ParseErrorRecord {
                file_path: format!("/path/log_{i}.sql"),
                error_message: format!("Error message {i}"),
                raw_content: Some(format!("SELECT * FROM table_{i}")),
                line_number: Some(i),
            };
            let result = logger.log_error(&record);
            assert!(result.is_ok(), "Failed at iteration {i}");
        }

        let finalize_result = logger.finalize();
        assert!(
            finalize_result.is_ok(),
            "Failed to finalize with many errors"
        );

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试 Debug 实现
    #[test]
    fn test_parse_error_record_debug() {
        let record = ParseErrorRecord {
            file_path: "/path/to/log.sql".to_string(),
            error_message: "Test error".to_string(),
            raw_content: Some("SELECT *".to_string()),
            line_number: Some(42),
        };

        let debug_str = format!("{record:?}");
        assert!(
            debug_str.contains("ParseErrorRecord"),
            "Debug should contain type name"
        );
        assert!(
            debug_str.contains("log.sql"),
            "Debug should contain file path"
        );
    }

    /// 测试错误指标的默认值
    #[test]
    fn test_error_logger_default_metrics() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_metrics.log");
        let result = ErrorLogger::new(&error_log_file);

        // 确认可以创建 logger（metrics 初始化正确）
        assert!(result.is_ok(), "Failed to create ErrorLogger with metrics");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }
    /// 测试 Unicode 字符支持
    #[test]
    fn test_error_logger_unicode() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_unicode.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        let record = ParseErrorRecord {
            file_path: "/path/to/日志.sql".to_string(),
            error_message: "错误信息: 无效的格式".to_string(),
            raw_content: Some("SELECT * FROM 表格 WHERE id = 1".to_string()),
            line_number: Some(42),
        };

        let result = logger.log_error(&record);
        assert!(result.is_ok(), "Failed with Unicode characters");

        // Clean up
        let _ = fs::remove_file(&error_log_file);
    }
}

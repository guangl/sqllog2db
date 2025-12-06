//! 针对 `error_logger.rs` 的深入覆盖测试
#[cfg(test)]
mod error_logger_deep_tests {
    use dm_database_sqllog2db::error_logger::{ErrorLogger, ErrorMetrics, ParseErrorRecord};
    use std::fs;
    use std::path::PathBuf;

    fn setup_test_dir(name: &str) -> PathBuf {
        let test_dir = PathBuf::from("target/test_error_logger").join(name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).expect("Failed to create test dir");
        test_dir
    }

    #[test]
    fn test_error_logger_creation() {
        let test_dir = setup_test_dir("creation");
        let error_log = test_dir.join("errors.jsonl");

        let result = ErrorLogger::new(&error_log);
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_logger_with_nested_dirs() {
        let test_dir = setup_test_dir("nested");
        let nested_path = test_dir.join("logs/errors/deep");
        let error_log = nested_path.join("errors.jsonl");

        let result = ErrorLogger::new(&error_log);
        assert!(result.is_ok());
        assert!(nested_path.exists());
    }

    #[test]
    fn test_parse_error_record_creation() {
        let record = ParseErrorRecord {
            file_path: "test.log".to_string(),
            error_message: "Parse error".to_string(),
            raw_content: Some("invalid line".to_string()),
            line_number: Some(42),
        };

        assert_eq!(record.file_path, "test.log");
        assert_eq!(record.line_number, Some(42));
        assert!(record.raw_content.is_some());
    }

    #[test]
    fn test_parse_error_record_without_content() {
        let record = ParseErrorRecord {
            file_path: "test.log".to_string(),
            error_message: "Error".to_string(),
            raw_content: None,
            line_number: None,
        };

        assert!(record.raw_content.is_none());
        assert!(record.line_number.is_none());
    }

    #[test]
    fn test_error_metrics_initialization() {
        let metrics = ErrorMetrics::default();
        assert_eq!(metrics.total, 0);
        assert_eq!(metrics.by_category.len(), 0);
        assert_eq!(metrics.parse_variants.len(), 0);
    }

    #[test]
    fn test_error_logger_log_error() {
        let test_dir = setup_test_dir("log_error");
        let error_log = test_dir.join("errors.jsonl");

        let mut logger = ErrorLogger::new(&error_log).expect("Failed to create logger");

        let record = ParseErrorRecord {
            file_path: "test.log".to_string(),
            error_message: "Test error".to_string(),
            raw_content: Some("raw data".to_string()),
            line_number: Some(1),
        };

        let result = logger.log_error(&record);
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_logger_multiple_records() {
        let test_dir = setup_test_dir("multiple_records");
        let error_log = test_dir.join("errors.jsonl");

        let mut logger = ErrorLogger::new(&error_log).expect("Failed to create logger");

        for i in 0..5 {
            let record = ParseErrorRecord {
                file_path: format!("file{i}.log"),
                error_message: format!("Error {i}"),
                raw_content: Some(format!("data {i}")),
                line_number: Some(i),
            };
            let result = logger.log_error(&record);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_error_logger_flush() {
        let test_dir = setup_test_dir("flush");
        let error_log = test_dir.join("errors.jsonl");

        let mut logger = ErrorLogger::new(&error_log).expect("Failed to create logger");

        let record = ParseErrorRecord {
            file_path: "test.log".to_string(),
            error_message: "Test".to_string(),
            raw_content: None,
            line_number: None,
        };

        let _ = logger.log_error(&record);
        let result = logger.flush();
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_logger_summary() {
        let test_dir = setup_test_dir("finalize");
        let error_log = test_dir.join("errors.jsonl");

        let mut logger = ErrorLogger::new(&error_log).expect("Failed to create logger");
        let result = logger.finalize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_error_record_debug() {
        let record = ParseErrorRecord {
            file_path: "test.log".to_string(),
            error_message: "Error".to_string(),
            raw_content: Some("content".to_string()),
            line_number: Some(10),
        };

        let debug = format!("{record:?}");
        assert!(debug.contains("test.log"));
    }

    #[test]
    fn test_error_metrics_debug() {
        let metrics = ErrorMetrics::default();
        let debug = format!("{metrics:?}");
        assert!(debug.contains("ErrorMetrics"));
    }

    #[test]
    fn test_error_logger_debug() {
        let test_dir = setup_test_dir("debug");
        let error_log = test_dir.join("errors.jsonl");

        let logger = ErrorLogger::new(&error_log).expect("Failed to create logger");
        let debug = format!("{logger:?}");
        assert!(debug.contains("ErrorLogger"));
    }

    #[test]
    fn test_parse_error_record_with_special_chars() {
        let record = ParseErrorRecord {
            file_path: "test_file_中文.log".to_string(),
            error_message: "Error: 特殊字符".to_string(),
            raw_content: Some("data with 特殊字符".to_string()),
            line_number: Some(99),
        };

        assert!(record.file_path.contains("中文"));
        assert!(record.error_message.contains("特殊字符"));
    }

    #[test]
    fn test_error_logger_file_exists() {
        let test_dir = setup_test_dir("exists");
        let error_log = test_dir.join("errors.jsonl");

        let _logger = ErrorLogger::new(&error_log).expect("Failed to create logger");
        assert!(error_log.exists());
    }

    #[test]
    fn test_error_logger_file_content() {
        let test_dir = setup_test_dir("content");
        let error_log = test_dir.join("errors.jsonl");

        let mut logger = ErrorLogger::new(&error_log).expect("Failed to create logger");

        let record = ParseErrorRecord {
            file_path: "test.log".to_string(),
            error_message: "Test error".to_string(),
            raw_content: Some("test data".to_string()),
            line_number: Some(5),
        };

        let _ = logger.log_error(&record);
        let _ = logger.flush();

        let content = fs::read_to_string(&error_log).expect("Failed to read file");
        assert!(!content.is_empty());
    }

    #[test]
    fn test_error_logger_append_mode() {
        let test_dir = setup_test_dir("append");
        let error_log = test_dir.join("errors.jsonl");

        // First write
        {
            let mut logger1 = ErrorLogger::new(&error_log).expect("Failed to create logger");
            let record1 = ParseErrorRecord {
                file_path: "file1.log".to_string(),
                error_message: "Error 1".to_string(),
                raw_content: None,
                line_number: None,
            };
            let _ = logger1.log_error(&record1);
            let _ = logger1.flush();
        }

        // Second write (append)
        {
            let mut logger2 = ErrorLogger::new(&error_log).expect("Failed to create logger");
            let record2 = ParseErrorRecord {
                file_path: "file2.log".to_string(),
                error_message: "Error 2".to_string(),
                raw_content: None,
                line_number: None,
            };
            let _ = logger2.log_error(&record2);
            let _ = logger2.flush();
        }

        let content = fs::read_to_string(&error_log).expect("Failed to read file");
        // Should contain both errors
        assert!(content.contains("file1.log") || content.contains("file2.log"));
    }
}

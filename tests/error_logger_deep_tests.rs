//! 针对 `error_logger.rs` 的深入覆盖测试
#[cfg(test)]
mod error_logger_deep_tests {
    use dm_database_sqllog2db::error_logger::ErrorLogger;
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
    fn test_error_logger_finalize() {
        let test_dir = setup_test_dir("finalize");
        let error_log = test_dir.join("errors.jsonl");

        let mut logger = ErrorLogger::new(&error_log).expect("Failed to create logger");
        let result = logger.finalize();
        assert!(result.is_ok());
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
    fn test_error_logger_file_exists() {
        let test_dir = setup_test_dir("exists");
        let error_log = test_dir.join("errors.jsonl");

        let _logger = ErrorLogger::new(&error_log).expect("Failed to create logger");
        assert!(error_log.exists());
    }
}

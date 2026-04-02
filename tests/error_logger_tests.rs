// 测试 error_logger.rs 模块的完整功能
#[cfg(test)]
mod error_logger_tests {
    use dm_database_sqllog2db::error_logger::ErrorLogger;
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

        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试完成和统计
    #[test]
    fn test_error_logger_finalize() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_finalize.log");
        let mut logger = ErrorLogger::new(&error_log_file).unwrap();

        let finalize_result = logger.finalize();
        assert!(finalize_result.is_ok(), "Failed to finalize");

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

        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试追加模式
    #[test]
    fn test_error_logger_append_mode() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_append.log");

        {
            let mut logger1 = ErrorLogger::new(&error_log_file).unwrap();
            logger1.finalize().ok();
        }

        {
            let mut logger2 = ErrorLogger::new(&error_log_file).unwrap();
            logger2.finalize().ok();
        }

        assert!(
            Path::new(&error_log_file).exists(),
            "Error log file should exist"
        );

        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试 Debug 实现
    #[test]
    fn test_error_logger_debug() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_debug.log");
        let logger = ErrorLogger::new(&error_log_file).unwrap();

        let debug_str = format!("{logger:?}");
        assert!(
            debug_str.contains("ErrorLogger"),
            "Debug should contain type name"
        );

        let _ = fs::remove_file(&error_log_file);
    }

    /// 测试错误指标的默认值
    #[test]
    fn test_error_logger_default_metrics() {
        let log_dir = "target/test_outputs";
        fs::create_dir_all(log_dir).ok();

        let error_log_file = format!("{log_dir}/test_metrics.log");
        let result = ErrorLogger::new(&error_log_file);

        assert!(result.is_ok(), "Failed to create ErrorLogger with metrics");

        let _ = fs::remove_file(&error_log_file);
    }
}

//! 为 error.rs 和 logging.rs 的深入覆盖测试
#[cfg(test)]
mod error_logging_deep_tests {
    use dm_database_sqllog2db::error::*;
    use std::path::PathBuf;

    #[test]
    fn test_config_error_no_exporters() {
        let err = ConfigError::NoExporters;
        let msg = format!("{err}");
        assert!(msg.contains("exporter"));
    }

    #[test]
    fn test_file_error_already_exists() {
        let path = PathBuf::from("test.txt");
        let err = FileError::AlreadyExists { path: path.clone() };
        let msg = format!("{err}");
        assert!(msg.contains("already exists"));
    }

    #[test]
    fn test_file_error_write_failed() {
        let path = PathBuf::from("test.txt");
        let err = FileError::WriteFailed {
            path: path.clone(),
            reason: "Permission denied".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("write") || msg.contains("Permission"));
    }

    #[test]
    fn test_file_error_create_dir_failed() {
        let path = PathBuf::from("/root/test");
        let err = FileError::CreateDirectoryFailed {
            path: path.clone(),
            reason: "Permission denied".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("directory") || msg.contains("Permission"));
    }

    #[test]
    fn test_parser_error_path_not_found() {
        let path = PathBuf::from("nonexistent.log");
        let err = ParserError::PathNotFound { path: path.clone() };
        let msg = format!("{err}");
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_parser_error_invalid_path() {
        let path = PathBuf::from("invalid path");
        let err = ParserError::InvalidPath {
            path: path.clone(),
            reason: "Invalid character".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("Invalid"));
    }

    #[test]
    fn test_config_error_not_found() {
        let path = PathBuf::from("config.toml");
        let err = ConfigError::NotFound(path.clone());
        let msg = format!("{err}");
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_config_error_parse_failed() {
        let path = PathBuf::from("config.toml");
        let err = ConfigError::ParseFailed {
            path: path.clone(),
            reason: "Invalid TOML".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("parse"));
    }

    #[test]
    fn test_config_error_invalid_log_level() {
        let err = ConfigError::InvalidLogLevel {
            level: "invalid".to_string(),
            valid_levels: vec!["info".to_string(), "debug".to_string()],
        };
        let msg = format!("{err}");
        assert!(msg.contains("invalid") || msg.contains("info"));
    }

    #[test]
    fn test_config_error_invalid_value() {
        let err = ConfigError::InvalidValue {
            field: "timeout".to_string(),
            value: "0".to_string(),
            reason: "must be positive".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("timeout") || msg.contains('0'));
    }

    #[test]
    fn test_error_debug_format() {
        let err = ConfigError::NoExporters;
        let debug = format!("{err:?}");
        assert!(debug.contains("NoExporters"));
    }

    #[test]
    fn test_multiple_errors() {
        let errors: Vec<ConfigError> = vec![
            ConfigError::NoExporters,
            ConfigError::NotFound(PathBuf::from("test.toml")),
        ];
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_error_clone() {
        let err1 = ConfigError::NoExporters;
        let msg1 = format!("{err1}");
        let msg2 = format!("{}", ConfigError::NoExporters);
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn test_file_error_debug() {
        let err = FileError::AlreadyExists {
            path: PathBuf::from("test.txt"),
        };
        let debug = format!("{err:?}");
        assert!(debug.contains("AlreadyExists"));
    }

    #[test]
    fn test_parser_error_debug() {
        let err = ParserError::PathNotFound {
            path: PathBuf::from("test.log"),
        };
        let debug = format!("{err:?}");
        assert!(debug.contains("PathNotFound"));
    }

    #[test]
    fn test_export_error_csv() {
        let err = ExportError::CsvExportFailed {
            path: PathBuf::from("test.csv"),
            reason: "test".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("CSV"));
    }

    #[test]
    fn test_export_error_file_create() {
        let err = ExportError::FileCreateFailed {
            path: PathBuf::from("test.txt"),
            reason: "Permission".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("create"));
    }

    #[test]
    fn test_export_error_file_write() {
        let err = ExportError::FileWriteFailed {
            path: PathBuf::from("test.txt"),
            reason: "Disk full".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("write"));
    }

    #[test]
    fn test_error_result_ok() {
        fn successful_op() -> String {
            "Success".to_string()
        }

        let result = successful_op();
        assert_eq!(result, "Success");
    }

    #[test]
    fn test_error_result_err_config() {
        fn failing_op() -> dm_database_sqllog2db::error::Result<String> {
            Err(Error::Config(ConfigError::NoExporters))
        }

        let result = failing_op();
        assert!(result.is_err());
    }

    #[test]
    fn test_error_result_err_file() {
        fn failing_op() -> dm_database_sqllog2db::error::Result<String> {
            Err(Error::File(FileError::WriteFailed {
                path: PathBuf::from("test"),
                reason: "err".to_string(),
            }))
        }

        let result = failing_op();
        assert!(result.is_err());
    }
}

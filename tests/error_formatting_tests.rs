//! 错误类型和格式化测试
#[cfg(test)]
mod error_formatting_tests {
    use dm_database_sqllog2db::error::{ConfigError, Error, FileError, ParserError};
    use std::path::PathBuf;

    #[test]
    fn test_config_error_not_found() {
        let path = PathBuf::from("nonexistent.toml");
        let err = Error::Config(ConfigError::NotFound(path.clone()));

        let err_str = format!("{err:?}");
        assert!(err_str.contains("NotFound"));
        assert!(err_str.contains("nonexistent.toml"));
    }

    #[test]
    fn test_config_error_parse_failed() {
        let path = PathBuf::from("bad.toml");
        let err = Error::Config(ConfigError::ParseFailed {
            path: path.clone(),
            reason: "syntax error".to_string(),
        });

        let err_str = format!("{err:?}");
        assert!(err_str.contains("ParseFailed"));
        assert!(err_str.contains("syntax error"));
    }

    #[test]
    fn test_config_error_invalid_value() {
        let err = Error::Config(ConfigError::InvalidValue {
            field: "level".to_string(),
            value: "invalid".to_string(),
            reason: "unknown level".to_string(),
        });

        let err_str = format!("{err:?}");
        assert!(err_str.contains("InvalidValue"));
        assert!(err_str.contains("level"));
    }

    #[test]
    fn test_file_error_create_directory_failed() {
        let path = PathBuf::from("/invalid/path");
        let err = Error::File(FileError::CreateDirectoryFailed {
            path: path.clone(),
            reason: "permission denied".to_string(),
        });

        let err_str = format!("{err:?}");
        assert!(err_str.contains("CreateDirectoryFailed"));
    }

    #[test]
    fn test_parser_error_path_not_found() {
        let path = PathBuf::from("missing.log");
        let err = Error::Parser(ParserError::PathNotFound { path: path.clone() });

        let err_str = format!("{err:?}");
        assert!(err_str.contains("PathNotFound"));
        assert!(err_str.contains("missing.log"));
    }

    #[test]
    fn test_parser_error_read_dir_failed() {
        let path = PathBuf::from("/no/access");
        let err = Error::Parser(ParserError::ReadDirFailed {
            path: path.clone(),
            reason: "permission denied".to_string(),
        });

        let err_str = format!("{err:?}");
        assert!(err_str.contains("ReadDirFailed"));
    }

    #[test]
    fn test_parser_error_invalid_path() {
        let path = PathBuf::from("/dev/null");
        let err = Error::Parser(ParserError::InvalidPath {
            path: path.clone(),
            reason: "not a regular file".to_string(),
        });

        let err_str = format!("{err:?}");
        assert!(err_str.contains("InvalidPath"));
    }

    #[test]
    fn test_error_display() {
        let err = Error::Config(ConfigError::NotFound(PathBuf::from("test.toml")));
        let display_str = format!("{err}");

        // Should have some meaningful message
        assert!(!display_str.is_empty());
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::InvalidValue {
            field: "test".to_string(),
            value: "bad".to_string(),
            reason: "invalid".to_string(),
        };

        let display_str = format!("{err}");
        assert!(!display_str.is_empty());
    }

    #[test]
    fn test_file_error_display() {
        let err = FileError::CreateDirectoryFailed {
            path: PathBuf::from("/tmp"),
            reason: "test".to_string(),
        };

        let display_str = format!("{err}");
        assert!(!display_str.is_empty());
    }

    #[test]
    fn test_parser_error_display() {
        let err = ParserError::PathNotFound {
            path: PathBuf::from("test.log"),
        };

        let display_str = format!("{err}");
        assert!(!display_str.is_empty());
    }

    #[test]
    fn test_error_source() {
        let err = Error::Config(ConfigError::NotFound(PathBuf::from("test.toml")));

        // Test that error implements std::error::Error
        let _ = std::error::Error::source(&err);
    }

    #[test]
    fn test_multiple_error_types() {
        let errors = vec![
            Error::Config(ConfigError::NotFound(PathBuf::from("a.toml"))),
            Error::File(FileError::CreateDirectoryFailed {
                path: PathBuf::from("/tmp"),
                reason: "test".to_string(),
            }),
            Error::Parser(ParserError::PathNotFound {
                path: PathBuf::from("b.log"),
            }),
        ];

        for err in errors {
            let _ = format!("{err:?}");
            let _ = format!("{err}");
        }
    }

    #[test]
    fn test_error_from_config_error() {
        let config_err = ConfigError::NotFound(PathBuf::from("test.toml"));
        let err: Error = Error::Config(config_err);

        assert!(matches!(err, Error::Config(_)));
    }

    #[test]
    fn test_error_from_file_error() {
        let file_err = FileError::CreateDirectoryFailed {
            path: PathBuf::from("/tmp"),
            reason: "test".to_string(),
        };
        let err: Error = Error::File(file_err);

        assert!(matches!(err, Error::File(_)));
    }

    #[test]
    fn test_error_from_parser_error() {
        let parser_err = ParserError::PathNotFound {
            path: PathBuf::from("test.log"),
        };
        let err: Error = Error::Parser(parser_err);

        assert!(matches!(err, Error::Parser(_)));
    }
}

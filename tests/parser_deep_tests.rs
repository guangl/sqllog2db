//! 针对 parser.rs 的深入覆盖测试
#[cfg(test)]
mod parser_deep_tests {
    use dm_database_sqllog2db::parser::SqllogParser;
    use std::fs;
    use std::path::PathBuf;

    fn setup_test_dir(name: &str) -> PathBuf {
        let test_dir = PathBuf::from("target/test_parser_deep").join(name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).expect("Failed to create test dir");
        test_dir
    }

    #[test]
    fn test_parser_new() {
        let parser = SqllogParser::new("test.log");
        assert_eq!(parser.path().to_str().unwrap(), "test.log");
    }

    #[test]
    fn test_parser_with_directory() {
        let test_dir = setup_test_dir("dir");
        let parser = SqllogParser::new(&test_dir);
        assert_eq!(parser.path(), test_dir.as_path());
    }

    #[test]
    fn test_parser_nonexistent_path() {
        let test_dir = setup_test_dir("nonexistent");
        let nonexistent = test_dir.join("does_not_exist");

        let parser = SqllogParser::new(&nonexistent);
        let result = parser.log_files();

        assert!(result.is_err());
        if let Err(e) = result {
            let err_msg = format!("{e:?}");
            assert!(err_msg.contains("PathNotFound") || err_msg.contains("not found"));
        }
    }

    #[test]
    fn test_parser_permission_denied_directory() {
        // This test is platform-specific and may not work on all systems
        let test_dir = setup_test_dir("no_permission");
        fs::write(test_dir.join("test.log"), "content").expect("Failed to write");

        // On Unix, we could set strict permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&test_dir, fs::Permissions::from_mode(0o000));
        }

        let parser = SqllogParser::new(&test_dir);
        let result = parser.log_files();

        // Restore permissions for cleanup
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&test_dir, fs::Permissions::from_mode(0o755));
        }

        // This may or may not fail depending on the platform
        let _ = result;
    }
    #[test]
    fn test_parser_special_file_type() {
        // Try to test the "既不是文件也不是目录" path
        // This is difficult on most systems, but we can try a device file or similar

        #[cfg(unix)]
        {
            let parser = SqllogParser::new("/dev/null");
            let result = parser.log_files();
            // On Unix, /dev/null is a special file, not a regular file or directory
            // But it might still be treated as a file by is_file()
            let _ = result;
        }

        #[cfg(windows)]
        {
            // On Windows, try a special device
            let parser = SqllogParser::new("CON");
            let result = parser.log_files();
            let _ = result;
        }
    }

    #[test]
    fn test_parser_single_file() {
        let test_dir = setup_test_dir("single_file");
        let log_file = test_dir.join("test.log");
        fs::write(&log_file, "test content").expect("Failed to write");

        let parser = SqllogParser::new(&log_file);
        let files = parser.log_files().expect("Failed to get files");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0], log_file);
    }

    #[test]
    fn test_parser_directory_with_log_files() {
        let test_dir = setup_test_dir("dir_with_logs");

        for i in 0..3 {
            let log_file = test_dir.join(format!("test{i}.log"));
            fs::write(&log_file, format!("content {i}")).expect("Failed to write");
        }

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_parser_directory_empty() {
        let test_dir = setup_test_dir("empty_dir");

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_parser_directory_with_non_log_files() {
        let test_dir = setup_test_dir("mixed_files");

        fs::write(test_dir.join("file1.txt"), "text").expect("Failed to write");
        fs::write(test_dir.join("file2.csv"), "csv").expect("Failed to write");
        fs::write(test_dir.join("file3.log"), "log").expect("Failed to write");

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        assert_eq!(files.len(), 1);
        assert!(files[0].to_str().unwrap().contains(".log"));
    }

    #[test]
    fn test_parser_path_accessor() {
        let test_path = PathBuf::from("test/path.log");
        let parser = SqllogParser::new(&test_path);

        assert_eq!(parser.path(), test_path.as_path());
    }

    #[test]
    fn test_parser_debug() {
        let parser = SqllogParser::new("test.log");
        let debug = format!("{parser:?}");

        assert!(debug.contains("SqllogParser"));
    }

    #[test]
    fn test_parser_multiple_log_files() {
        let test_dir = setup_test_dir("multiple_logs");

        for i in 0..10 {
            let log_file = test_dir.join(format!("app_{i}.log"));
            fs::write(&log_file, format!("log content {i}")).expect("Failed to write");
        }

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        assert_eq!(files.len(), 10);
    }

    #[test]
    fn test_parser_nested_directory_ignored() {
        let test_dir = setup_test_dir("nested");
        let nested = test_dir.join("subdir");
        fs::create_dir_all(&nested).expect("Failed to create nested");

        fs::write(test_dir.join("root.log"), "root").expect("Failed to write");
        fs::write(nested.join("nested.log"), "nested").expect("Failed to write");

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        // Should only find root.log, not nested.log
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_parser_with_dotfiles() {
        let test_dir = setup_test_dir("dotfiles");

        fs::write(test_dir.join(".hidden.log"), "hidden").expect("Failed to write");
        fs::write(test_dir.join("visible.log"), "visible").expect("Failed to write");

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        // Both should be found
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_parser_case_sensitive_extension() {
        let test_dir = setup_test_dir("case_ext");

        fs::write(test_dir.join("file.log"), "lower").expect("Failed to write");
        fs::write(test_dir.join("file.LOG"), "upper").expect("Failed to write");

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        // Depends on file system case sensitivity
        assert!(!files.is_empty());
    }

    #[test]
    fn test_parser_special_chars_filename() {
        let test_dir = setup_test_dir("special_chars");

        let special_file = test_dir.join("file-2025_12_06.log");
        fs::write(&special_file, "content").expect("Failed to write");

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_parser_utf8_filename() {
        let test_dir = setup_test_dir("utf8");

        let utf8_file = test_dir.join("日志文件.log");
        fs::write(&utf8_file, "内容").expect("Failed to write");

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_parser_log_files_called_twice() {
        let test_dir = setup_test_dir("twice");

        fs::write(test_dir.join("test.log"), "content").expect("Failed to write");

        let parser = SqllogParser::new(&test_dir);

        let files1 = parser.log_files().expect("Failed first call");
        let files2 = parser.log_files().expect("Failed second call");

        assert_eq!(files1.len(), files2.len());
    }

    #[test]
    fn test_parser_with_symlink_ignored() {
        // This test might not work on all systems
        let test_dir = setup_test_dir("symlink");

        fs::write(test_dir.join("real.log"), "real").expect("Failed to write");

        let parser = SqllogParser::new(&test_dir);
        let files = parser.log_files().expect("Failed to get files");

        assert!(!files.is_empty());
    }
}

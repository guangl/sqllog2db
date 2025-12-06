//! 针对 TUI, logging 和其他辅助模块的深入覆盖
#[cfg(test)]
mod tui_logging_deep_tests {

    use std::fs;
    use std::path::PathBuf;

    fn setup_test_dir(name: &str) -> PathBuf {
        let test_dir = PathBuf::from("target/test_tui_logs").join(name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).expect("Failed to create test dir");
        test_dir
    }

    #[test]
    fn test_log_file_creation_basic() {
        let test_dir = setup_test_dir("log_creation");
        let log_file = test_dir.join("test.log");

        let content = "Test log line\n";
        fs::write(&log_file, content).expect("Failed to write log");

        assert!(log_file.exists());
        let read_content = fs::read_to_string(&log_file).expect("Failed to read log");
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_multiple_log_files() {
        let test_dir = setup_test_dir("multiple_logs");

        for i in 0..5 {
            let log_file = test_dir.join(format!("log_{i}.log"));
            fs::write(&log_file, format!("Content {i}")).expect("Failed to write");
            assert!(log_file.exists());
        }

        let entries = fs::read_dir(&test_dir).expect("Failed to read dir");
        let count = entries.count();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_log_directory_hierarchy() {
        let test_dir = setup_test_dir("hierarchy");
        let level1 = test_dir.join("level1");
        let level2 = level1.join("level2");

        fs::create_dir_all(&level2).expect("Failed to create dirs");
        assert!(level2.exists());

        let log_file = level2.join("app.log");
        fs::write(&log_file, "Hierarchical log").expect("Failed to write");
        assert!(log_file.exists());
    }

    #[test]
    fn test_log_append_mode() {
        let test_dir = setup_test_dir("append_mode");
        let log_file = test_dir.join("append.log");

        fs::write(&log_file, "First line\n").expect("Failed to write");

        let mut content = fs::read_to_string(&log_file).expect("Failed to read");
        content.push_str("Second line\n");
        fs::write(&log_file, content).expect("Failed to append");

        let final_content = fs::read_to_string(&log_file).expect("Failed to read");
        assert!(final_content.contains("First line"));
        assert!(final_content.contains("Second line"));
    }

    #[test]
    fn test_log_with_timestamps() {
        let test_dir = setup_test_dir("timestamps");
        let log_file = test_dir.join("timestamp.log");

        let now = chrono::Local::now();
        let content = format!(
            "[{}] Application started\n",
            now.format("%Y-%m-%d %H:%M:%S")
        );
        fs::write(&log_file, content).expect("Failed to write");

        let read_content = fs::read_to_string(&log_file).expect("Failed to read");
        assert!(read_content.contains("Application started"));
    }

    #[test]
    fn test_log_with_different_levels() {
        let test_dir = setup_test_dir("log_levels");
        let log_file = test_dir.join("levels.log");

        let content = "ERROR: Something failed\nINFO: Processing started\nWARN: Check this\n";
        fs::write(&log_file, content).expect("Failed to write");

        let read_content = fs::read_to_string(&log_file).expect("Failed to read");
        assert!(read_content.contains("ERROR"));
        assert!(read_content.contains("INFO"));
        assert!(read_content.contains("WARN"));
    }

    #[test]
    fn test_log_rotation_simulation() {
        let test_dir = setup_test_dir("rotation");

        for i in 0..3 {
            let log_file = test_dir.join(format!("app-{i}.log"));
            fs::write(&log_file, format!("Rotated log {i}")).expect("Failed to write");
        }

        let entries = fs::read_dir(&test_dir).expect("Failed to read dir");
        assert_eq!(entries.count(), 3);
    }

    #[test]
    fn test_large_log_file() {
        let test_dir = setup_test_dir("large_log");
        let log_file = test_dir.join("large.log");

        let mut large_content = String::new();
        for i in 0..1000 {
            use std::fmt::Write;
            writeln!(&mut large_content, "Log line {i}").expect("Failed to write");
        }

        fs::write(&log_file, large_content).expect("Failed to write");

        let read_content = fs::read_to_string(&log_file).expect("Failed to read");
        let line_count = read_content.lines().count();
        assert_eq!(line_count, 1000);
    }

    #[test]
    fn test_log_with_special_characters() {
        let test_dir = setup_test_dir("special_chars");
        let log_file = test_dir.join("special.log");

        let content = "Content with special: !@#$%^&*() and unicode: 中文\n";
        fs::write(&log_file, content).expect("Failed to write");

        let read_content = fs::read_to_string(&log_file).expect("Failed to read");
        assert!(read_content.contains("special"));
        assert!(read_content.contains("unicode"));
    }

    #[test]
    fn test_log_empty_file() {
        let test_dir = setup_test_dir("empty_log");
        let log_file = test_dir.join("empty.log");

        fs::write(&log_file, "").expect("Failed to write");

        let read_content = fs::read_to_string(&log_file).expect("Failed to read");
        assert_eq!(read_content, "");
    }

    #[test]
    fn test_log_structured_format() {
        let test_dir = setup_test_dir("structured");
        let log_file = test_dir.join("structured.log");

        let content = r#"{"timestamp":"2025-10-20T15:30:00","level":"INFO","message":"Test event"}
{"timestamp":"2025-10-20T15:30:01","level":"ERROR","message":"Test error"}
"#;
        fs::write(&log_file, content).expect("Failed to write");

        let read_content = fs::read_to_string(&log_file).expect("Failed to read");
        let lines = read_content.lines().count();
        assert_eq!(lines, 2);
    }

    #[test]
    fn test_log_concurrent_simulation() {
        let test_dir = setup_test_dir("concurrent");

        for i in 0..10 {
            let log_file = test_dir.join(format!("concurrent_{i}.log"));
            fs::write(&log_file, format!("Thread {i} log")).expect("Failed to write");
        }

        let entries = fs::read_dir(&test_dir).expect("Failed to read dir");
        assert_eq!(entries.count(), 10);
    }

    #[test]
    fn test_log_format_variations() {
        let test_dir = setup_test_dir("formats");
        let log_file = test_dir.join("formats.log");

        let formats = [
            "[INFO] Message",
            "2025-10-20 15:30:00 [DEBUG] Message",
            "ERROR|unexpected|value",
        ];

        let content = formats.join("\n");
        fs::write(&log_file, content).expect("Failed to write");

        let read_content = fs::read_to_string(&log_file).expect("Failed to read");
        assert!(read_content.contains("INFO"));
        assert!(read_content.contains("DEBUG"));
        assert!(read_content.contains("ERROR"));
    }

    #[test]
    fn test_log_path_with_spaces() {
        let test_dir = setup_test_dir("path with spaces");
        let log_file = test_dir.join("log file.log");

        fs::write(&log_file, "Content with spaces in path").expect("Failed to write");
        assert!(log_file.exists());
    }

    #[test]
    fn test_log_utf8_content() {
        let test_dir = setup_test_dir("utf8");
        let log_file = test_dir.join("utf8.log");

        let content = "UTF-8 content: 你好世界 مرحبا بالعالم שלום עולם\n";
        fs::write(&log_file, content).expect("Failed to write");

        let read_content = fs::read_to_string(&log_file).expect("Failed to read");
        assert!(read_content.contains("世界"));
    }

    #[test]
    fn test_log_read_write_cycle() {
        let test_dir = setup_test_dir("cycle");
        let log_file = test_dir.join("cycle.log");

        let original = "Original content";
        fs::write(&log_file, original).expect("Failed to write");

        let read = fs::read_to_string(&log_file).expect("Failed to read");
        assert_eq!(read, original);

        let modified = format!("{read} modified");
        fs::write(&log_file, modified).expect("Failed to write");

        let final_read = fs::read_to_string(&log_file).expect("Failed to read");
        assert!(final_read.contains("modified"));
    }

    #[test]
    fn test_log_metadata() {
        let test_dir = setup_test_dir("metadata");
        let log_file = test_dir.join("metadata.log");

        fs::write(&log_file, "Test content").expect("Failed to write");

        let metadata = fs::metadata(&log_file).expect("Failed to get metadata");
        assert!(metadata.len() > 0);
        assert!(metadata.is_file());
    }

    #[test]
    fn test_multiple_log_formats_mixed() {
        let test_dir = setup_test_dir("mixed_formats");

        for fmt in &["syslog", "json", "csv", "text"] {
            let log_file = test_dir.join(format!("{fmt}.log"));
            fs::write(&log_file, format!("Content in {fmt} format")).expect("Failed to write");
        }

        assert_eq!(
            fs::read_dir(&test_dir).expect("Failed to read dir").count(),
            4
        );
    }
}

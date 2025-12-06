/// Parser module tests
use dm_database_sqllog2db::parser::SqllogParser;
use std::path::PathBuf;

// ==================== SqllogParser Basic Tests ====================

#[test]
fn test_sqllog_parser_new() {
    let path = PathBuf::from("sqllogs");
    let parser = SqllogParser::new(&path);
    assert_eq!(parser.path(), path.as_path());
}

#[test]
fn test_sqllog_parser_new_with_string() {
    let parser = SqllogParser::new("sqllogs/sample.log");
    assert_eq!(parser.path().to_string_lossy(), "sqllogs/sample.log");
}

#[test]
fn test_sqllog_parser_path() {
    let path = PathBuf::from("data/logs");
    let parser = SqllogParser::new(&path);
    let retrieved_path = parser.path();
    assert_eq!(retrieved_path, path.as_path());
}

#[test]
fn test_sqllog_parser_path_absolute() {
    let path = PathBuf::from("C:\\logs\\sql.log");
    let parser = SqllogParser::new(&path);
    assert_eq!(parser.path(), path.as_path());
}

// ==================== SqllogParser Error Cases ====================

#[test]
fn test_sqllog_parser_nonexistent_path() {
    let parser = SqllogParser::new("nonexistent/path/to/logs");
    let result = parser.log_files();
    assert!(result.is_err());
}

#[test]
fn test_sqllog_parser_nonexistent_file() {
    let parser = SqllogParser::new("nonexistent_file.log");
    let result = parser.log_files();
    assert!(result.is_err());
}

// ==================== SqllogParser Debug Implementation ====================

#[test]
fn test_sqllog_parser_debug() {
    let parser = SqllogParser::new("sqllogs");
    let debug_str = format!("{parser:?}");
    assert!(debug_str.contains("SqllogParser"));
    assert!(debug_str.contains("sqllogs"));
}

#[test]
fn test_sqllog_parser_debug_with_complex_path() {
    let parser = SqllogParser::new("C:\\Users\\Admin\\Logs\\Database");
    let debug_str = format!("{parser:?}");
    assert!(debug_str.contains("SqllogParser"));
}

// ==================== SqllogParser Directory Scanning Tests ====================

#[test]
fn test_sqllog_parser_scan_directory_with_log_files() {
    // Create temporary directory with log files
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Create some test .log files
    std::fs::write(temp_path.join("test1.log"), "test").expect("Failed to write test1.log");
    std::fs::write(temp_path.join("test2.log"), "test").expect("Failed to write test2.log");
    std::fs::write(temp_path.join("test.txt"), "test").expect("Failed to write test.txt");

    let parser = SqllogParser::new(temp_path);
    let result = parser.log_files();

    assert!(result.is_ok(), "Should successfully scan directory");
    let files = result.unwrap();
    assert_eq!(files.len(), 2, "Should find exactly 2 .log files");

    // Verify that .txt files are not included
    for file in &files {
        assert!(
            file.extension().is_some_and(|ext| ext == "log"),
            "Only .log files should be included"
        );
    }
}

#[test]
fn test_sqllog_parser_scan_empty_directory() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    let parser = SqllogParser::new(temp_path);
    let result = parser.log_files();

    assert!(result.is_ok(), "Should succeed even for empty directory");
    let files = result.unwrap();
    assert_eq!(
        files.len(),
        0,
        "Should find 0 .log files in empty directory"
    );
}

#[test]
fn test_sqllog_parser_scan_directory_no_log_files() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Create only non-.log files
    std::fs::write(temp_path.join("file.txt"), "test").expect("Failed to write file.txt");
    std::fs::write(temp_path.join("data.csv"), "test").expect("Failed to write data.csv");

    let parser = SqllogParser::new(temp_path);
    let result = parser.log_files();

    assert!(
        result.is_ok(),
        "Should succeed for directory with no .log files"
    );
    let files = result.unwrap();
    assert_eq!(files.len(), 0, "Should find 0 .log files");
}

#[test]
fn test_sqllog_parser_scan_single_file() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();
    let log_file = temp_path.join("single.log");

    std::fs::write(&log_file, "test content").expect("Failed to write log file");

    let parser = SqllogParser::new(&log_file);
    let result = parser.log_files();

    assert!(result.is_ok(), "Should successfully scan single file");
    let files = result.unwrap();
    assert_eq!(files.len(), 1, "Should find exactly 1 file");
    assert_eq!(files[0], log_file, "Should return the exact file path");
}

#[test]
fn test_sqllog_parser_invalid_path_type() {
    // This test would require special setup on different OSes
    // For now we test with a symbolic link or other special file if available
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Create a regular file
    let file_path = temp_path.join("test.file");
    std::fs::write(&file_path, "test").expect("Failed to write test file");

    // On the actual file, it should work (is_file returns true)
    let parser = SqllogParser::new(&file_path);
    let result = parser.log_files();
    assert!(result.is_ok(), "Regular file should be handled correctly");
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn setup_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_filter_refactor_outputs").join(name);
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    test_dir
}

fn get_binary_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .join("target")
        .join("debug")
        .join(if cfg!(windows) {
            "sqllog2db.exe"
        } else {
            "sqllog2db"
        })
}

fn create_sample_logs(sqllog_dir: &Path) {
    // Transaction 5001: Slow query (1500ms)
    let log1 = r"2025-10-20 15:10:28.614 (EP[0] sess:0x1 user:U1 trxid:5001 stmt:0x1 appname: ip:1.1.1.1) [INS] INSERT START 5001.
2025-10-20 15:10:28.615 (EP[0] sess:0x1 user:U1 trxid:5001 stmt:0x2 appname: ip:1.1.1.1) [INS] INSERT DATA 5001. EXECTIME: 1500(ms) ROWCOUNT: 1(rows) EXEC_ID: 5001.
";
    // Transaction 5002: Fast query (10ms) but many rows (500)
    let log2 = r"2025-10-20 15:10:28.616 (EP[0] sess:0x2 user:U1 trxid:5002 stmt:0x3 appname: ip:1.1.1.1) [INS] INSERT DATA 5002. EXECTIME: 10(ms) ROWCOUNT: 500(rows) EXEC_ID: 5002.
2025-10-20 15:10:28.617 (EP[0] sess:0x2 user:U1 trxid:5002 stmt:0x4 appname: ip:1.1.1.1) [INS] COMMIT 5002.
";
    // Transaction 5003: Fast query (10ms) and few rows (1)
    let log3 = r"2025-10-20 15:10:28.618 (EP[0] sess:0x3 user:U1 trxid:5003 stmt:0x5 appname: ip:1.1.1.1) [INS] INSERT 5003. EXECTIME: 10(ms) ROWCOUNT: 1(rows) EXEC_ID: 5003.
";
    fs::write(sqllog_dir.join("log1.log"), log1).expect("Failed to write log1");
    fs::write(sqllog_dir.join("log2.log"), log2).expect("Failed to write log2");
    fs::write(sqllog_dir.join("log3.log"), log3).expect("Failed to write log3");
}

#[test]
fn test_min_runtime_filtering() {
    let test_dir = setup_test_dir("min_runtime");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_csv = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    create_sample_logs(&sqllog_dir);

    let binary = get_binary_path();

    let config_content = format!(
        r#"[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features.filters]
enable = true

[features.filters.indicators]
min_runtime_ms = 1000

[exporter.csv]
file = "{}"
overwrite = true
append = false
"#,
        sqllog_dir.to_string_lossy().replace('\\', "/"),
        test_dir
            .join("errors.log")
            .to_string_lossy()
            .replace('\\', "/"),
        test_dir
            .join("app.log")
            .to_string_lossy()
            .replace('\\', "/"),
        output_csv.to_string_lossy().replace('\\', "/"),
    );
    fs::write(&config_path, config_content).expect("Failed to write config");

    let run_output = Command::new(&binary)
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute run");

    assert!(run_output.status.success());

    let output_content = fs::read_to_string(&output_csv).expect("Failed to read output csv");
    assert!(output_content.contains("5001"));
    assert!(output_content.contains("INSERT START 5001"));
    assert!(!output_content.contains("5002"));
    assert!(!output_content.contains("5003"));
}

#[test]
fn test_min_row_count_filtering() {
    let test_dir = setup_test_dir("min_row_count");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_csv = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    create_sample_logs(&sqllog_dir);

    let binary = get_binary_path();

    let config_content = format!(
        r#"[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features.filters]
enable = true

[features.filters.indicators]
min_row_count = 100

[exporter.csv]
file = "{}"
overwrite = true
append = false
"#,
        sqllog_dir.to_string_lossy().replace('\\', "/"),
        test_dir
            .join("errors.log")
            .to_string_lossy()
            .replace('\\', "/"),
        test_dir
            .join("app.log")
            .to_string_lossy()
            .replace('\\', "/"),
        output_csv.to_string_lossy().replace('\\', "/"),
    );
    fs::write(&config_path, config_content).expect("Failed to write config");

    let run_output = Command::new(&binary)
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute run");

    assert!(run_output.status.success());

    let output_content = fs::read_to_string(&output_csv).expect("Failed to read output csv");
    assert!(!output_content.contains("5001"));
    assert!(output_content.contains("5002"));
    assert!(output_content.contains("COMMIT 5002"));
    assert!(!output_content.contains("5003"));
}

use std::{fs, path::PathBuf, process::Command};

// Helper to get path to compiled binary provided by Cargo for integration tests
fn binary_path() -> PathBuf {
    // 使用 cargo 测试时提供的二进制路径
    let exe = env!("CARGO_BIN_EXE_sqllog2db");
    PathBuf::from(exe)
}

// 转换路径为 TOML 兼容格式（Windows 反斜杠转义）
fn toml_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

#[test]
fn run_with_empty_logs_creates_csv_header_only() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();

    // Prepare paths
    let logs_dir = work_dir.join("logs_input");
    fs::create_dir_all(&logs_dir).unwrap();
    let out_csv = work_dir.join("out.csv");
    let log_file = work_dir.join("app.log");
    let config_path = work_dir.join("config.toml");

    // Write config pointing to empty logs directory and a CSV exporter
    let cfg = format!(
        r#"
[sqllog]
path = "{}"
thread_count = 0

[error]
path = "{}"

[logging]
path = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[[exporter.csv]]
path = "{}"
overwrite = true
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.jsonl")),
        toml_path(&log_file),
        toml_path(&out_csv)
    );
    fs::write(&config_path, cfg).unwrap();

    // Invoke binary: sqllog2db run -c <config>
    let status = Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(config_path.as_os_str())
        .status()
        .expect("failed to run binary");

    assert!(
        status.success(),
        "binary exit status not success: {:?}",
        status
    );

    // CSV should exist and contain only header (1 line)
    let content = fs::read_to_string(&out_csv).expect("csv not created");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1, "expected only header line when no logs");
    assert!(lines[0].starts_with("timestamp,ep,sess_id"));
}

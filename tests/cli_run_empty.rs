use std::{fs, path::PathBuf, process::Command};

// Helper to get path to compiled binary provided by Cargo for integration tests
fn binary_path() -> PathBuf {
    // CARGO_BIN_EXE_<name> is set by Cargo for each binary target
    let var = format!("CARGO_BIN_EXE_{}", env!("CARGO_PKG_NAME"));
    let path = std::env::var(&var).expect("binary path env not set by cargo");
    PathBuf::from(path)
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

[[exporter.csv]]
path = "{}"
overwrite = true
"#,
        logs_dir.display(),
        work_dir.join("errors.jsonl").display(),
        log_file.display(),
        out_csv.display()
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

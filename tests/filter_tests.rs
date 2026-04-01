#![cfg(feature = "filters")]
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn setup_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_filter_outputs").join(name);
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
    let log1 = r"2025-10-20 15:10:28.614 (EP[0] sess:0x1 user:U1 trxid:101 stmt:0x1 appname: ip:1.1.1.1) [INS] INSERT 101.
";
    let log2 = r"2025-10-20 15:10:28.615 (EP[0] sess:0x2 user:U1 trxid:102 stmt:0x2 appname: ip:1.1.1.1) [INS] INSERT 102.
";
    let log3 = r"2025-10-20 15:10:28.616 (EP[0] sess:0x3 user:U1 trxid:103 stmt:0x3 appname: ip:1.1.1.1) [INS] INSERT 103.
";
    fs::write(sqllog_dir.join("log1.log"), log1).expect("Failed to write log1");
    fs::write(sqllog_dir.join("log2.log"), log2).expect("Failed to write log2");
    fs::write(sqllog_dir.join("log3.log"), log3).expect("Failed to write log3");
}

#[test]
fn test_trxid_filtering() {
    let test_dir = setup_test_dir("trxid_filter");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_csv = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    create_sample_logs(&sqllog_dir);

    let binary = get_binary_path();

    // Create config with trxid filter
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
trxids = ["101", "103"]

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

    // Run export
    let run_output = Command::new(&binary)
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute run");

    if !run_output.status.success() {
        eprintln!("STDOUT: {}", String::from_utf8_lossy(&run_output.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&run_output.stderr));
    }
    assert!(run_output.status.success());

    // Verify output contains only trxid 101 and 103
    let output_content = fs::read_to_string(&output_csv).expect("Failed to read output csv");
    // CSV format depends on implementation, but it should contain "101" and "103" and NOT "102"
    assert!(output_content.contains("101"));
    assert!(output_content.contains("103"));
    assert!(!output_content.contains("102"));
}

#[test]
fn test_no_trxid_filtering() {
    let test_dir = setup_test_dir("no_trxid_filter");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_csv = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    create_sample_logs(&sqllog_dir);

    let binary = get_binary_path();

    // Create config WITHOUT trxid filter
    let config_content = format!(
        r#"[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

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

    // Run export
    let run_output = Command::new(&binary)
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute run");

    if !run_output.status.success() {
        eprintln!("STDOUT: {}", String::from_utf8_lossy(&run_output.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&run_output.stderr));
    }
    assert!(run_output.status.success());

    // Verify output contains all trxids
    let output_content = fs::read_to_string(&output_csv).expect("Failed to read output csv");
    assert!(output_content.contains("101"));
    assert!(output_content.contains("102"));
    assert!(output_content.contains("103"));
}

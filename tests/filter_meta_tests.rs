#![cfg(feature = "filters")]
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn setup_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_filter_meta_outputs").join(name);
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
    let log1 = r"2025-10-20 15:10:28.614 (EP[0] sess:0xSESSION1 thrd:THREAD1 user:USER1 trxid:4001 stmt:INS appname:APP1 ip:1.1.1.1) [INS] INSERT 1.
";
    let log2 = r"2025-10-20 15:10:28.615 (EP[0] sess:0xSESSION2 thrd:THREAD2 user:USER2 trxid:4002 stmt:UPD appname:APP2 ip:2.2.2.2) [UPD] UPDATE 2.
";
    let log3 = r"2025-10-20 15:10:28.616 (EP[0] sess:0xSESSION3 thrd:THREAD3 user:USER3 trxid:4003 stmt:SEL appname:APP3 ip:3.3.3.3) [SEL] SELECT 3.
";
    fs::write(sqllog_dir.join("log1.log"), log1).expect("Failed to write log1");
    fs::write(sqllog_dir.join("log2.log"), log2).expect("Failed to write log2");
    fs::write(sqllog_dir.join("log3.log"), log3).expect("Failed to write log3");
}

#[test]
fn test_sess_id_filtering() {
    let test_dir = setup_test_dir("sess_filter");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_csv = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    create_sample_logs(&sqllog_dir);

    let binary = get_binary_path();

    let config_content = format!(
        r#"[sqllog]
directory = "{}"

[features.filters]
enable = true
sess_ids = ["SESSION1"]

[exporter.csv]
file = "{}"
overwrite = true
append = false
"#,
        sqllog_dir.to_string_lossy().replace('\\', "/"),
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
    assert!(output_content.contains("4001"));
    assert!(!output_content.contains("4002"));
}

#[test]
fn test_thrd_id_filtering() {
    let test_dir = setup_test_dir("thrd_filter");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_csv = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    create_sample_logs(&sqllog_dir);

    let binary = get_binary_path();

    let config_content = format!(
        r#"[sqllog]
directory = "{}"

[features.filters]
enable = true
thrd_ids = ["THREAD2"]

[exporter.csv]
file = "{}"
overwrite = true
append = false
"#,
        sqllog_dir.to_string_lossy().replace('\\', "/"),
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
    assert!(output_content.contains("4002"));
    assert!(!output_content.contains("4001"));
}

#[test]
fn test_user_filtering() {
    let test_dir = setup_test_dir("user_filter");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_csv = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    create_sample_logs(&sqllog_dir);

    let binary = get_binary_path();

    let config_content = format!(
        r#"[sqllog]
directory = "{}"

[features.filters]
enable = true
usernames = ["USER3"]

[exporter.csv]
file = "{}"
overwrite = true
append = false
"#,
        sqllog_dir.to_string_lossy().replace('\\', "/"),
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
    assert!(output_content.contains("4003"));
    assert!(!output_content.contains("4001"));
}

#[test]
fn test_stmt_filtering() {
    let test_dir = setup_test_dir("stmt_filter");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_csv = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    create_sample_logs(&sqllog_dir);

    let binary = get_binary_path();

    let config_content = format!(
        r#"[sqllog]
directory = "{}"

[features.filters]
enable = true
statements = ["UPD", "SEL"]

[exporter.csv]
file = "{}"
overwrite = true
append = false
"#,
        sqllog_dir.to_string_lossy().replace('\\', "/"),
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
    assert!(output_content.contains("4002"));
    assert!(output_content.contains("4003"));
    assert!(!output_content.contains("4001"));
}

#[test]
fn test_appname_filtering() {
    let test_dir = setup_test_dir("appname_filter");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_csv = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    create_sample_logs(&sqllog_dir);

    let binary = get_binary_path();

    let config_content = format!(
        r#"[sqllog]
directory = "{}"

[features.filters]
enable = true
appnames = ["APP1"]

[exporter.csv]
file = "{}"
overwrite = true
append = false
"#,
        sqllog_dir.to_string_lossy().replace('\\', "/"),
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
    assert!(output_content.contains("4001"));
    assert!(!output_content.contains("4003"));
}

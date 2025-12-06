//! 额外的端到端 CLI 测试以达到更高覆盖率
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn setup_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_e2e_extra").join(name);
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

fn create_sample_log(log_file: &PathBuf) {
    let content = r"2025-10-20 15:10:28.614 (EP[0] sess:0x7f41435437a8 thrd:2188515 user:OASIS_MSG trxid:0 stmt:0x7f41435677a8 appname: ip:::ffff:10.63.97.88) [INS] INSERT INTO OASIS_MSG.SYS_NOTIFY_TODOTARGET VALUES( ?,?,? ) EXECTIME: 3(ms) ROWCOUNT: 1(rows) EXEC_ID: 257809109.
2025-10-20 15:10:28.615 (EP[0] sess:0x114475f8 thrd:2213103 user:SYSDBA trxid:0 stmt:0x1146b5f8 appname: ip:::ffff:10.63.97.89) [SEL] select client_id from oauth_client_details where client_id = ? EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 257809310.
";
    fs::write(log_file, content).expect("Failed to write log file");
}

#[test]
fn test_cli_init_default_output() {
    let test_dir = setup_test_dir("init_default");
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .arg("init")
        .current_dir(&test_dir)
        .output()
        .expect("Failed to execute init");

    assert!(output.status.success());

    let default_config = test_dir.join("config.toml");
    assert!(
        default_config.exists(),
        "Default config.toml should be created"
    );
}

#[test]
fn test_cli_init_with_force_creates_file() {
    let test_dir = setup_test_dir("init_force_creates");
    let config_path = test_dir.join("custom.toml");

    fs::write(&config_path, "old content").expect("Failed to create initial file");

    let binary = get_binary_path();

    let output = Command::new(&binary)
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .arg("--force")
        .output()
        .expect("Failed to execute init with force");

    assert!(output.status.success());
    assert!(config_path.exists());

    let content = fs::read_to_string(&config_path).expect("Failed to read config");
    assert!(
        content.contains("[sqllog]"),
        "Config should contain sqllog section"
    );
}

#[test]
fn test_cli_validate_with_invalid_level() {
    let test_dir = setup_test_dir("validate_bad_level");
    let config_path = test_dir.join("config.toml");

    let invalid_config = r#"
[sqllog]
directory = "sqllogs"

[error]
file = "errors.log"

[logging]
file = "app.log"
level = "invalid_level_xyz"
retention_days = 7

[features.replace_parameters]
enable = false

[exporter.csv]
file = "output.csv"
overwrite = true
"#;

    fs::write(&config_path, invalid_config).expect("Failed to write config");

    let binary = get_binary_path();
    let output = Command::new(&binary)
        .arg("validate")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute validate");

    assert!(
        !output.status.success(),
        "Should fail for invalid log level"
    );
}

#[test]
fn test_cli_run_with_config_and_verbose() {
    let test_dir = setup_test_dir("run_verbose_detailed");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    let log_file = sqllog_dir.join("sample.log");
    create_sample_log(&log_file);

    let binary = get_binary_path();

    let init_output = Command::new(&binary)
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .output()
        .expect("Failed to execute init");

    assert!(init_output.status.success());

    let mut config = fs::read_to_string(&config_path).expect("Failed to read config");
    let sqllog_display = sqllog_dir.to_string_lossy().to_string().replace('\\', "/");
    config = config.replace("sqllogs", &sqllog_display);
    fs::write(&config_path, config).expect("Failed to write config");

    let run_output = Command::new(&binary)
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .arg("--verbose")
        .output()
        .expect("Failed to execute run");

    assert!(run_output.status.success());

    let stderr = String::from_utf8_lossy(&run_output.stderr);
    assert!(!stderr.is_empty(), "Should produce output");
}

#[test]
fn test_cli_run_generates_output() {
    let test_dir = setup_test_dir("run_output_check");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");
    let output_file = test_dir.join("output.csv");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    let log_file = sqllog_dir.join("sample.log");
    create_sample_log(&log_file);

    let binary = get_binary_path();

    let init_output = Command::new(&binary)
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .output()
        .expect("Failed to execute init");

    assert!(init_output.status.success());

    let mut config = fs::read_to_string(&config_path).expect("Failed to read config");
    let sqllog_display = sqllog_dir.to_string_lossy().to_string().replace('\\', "/");
    config = config.replace("sqllogs", &sqllog_display);

    // Set output file path
    let output_display = output_file.to_string_lossy().to_string().replace('\\', "/");
    config = config.replace("outputs/sqllog.csv", &output_display);

    fs::write(&config_path, config).expect("Failed to write config");

    let run_output = Command::new(&binary)
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute run");

    assert!(run_output.status.success());
}

#[test]
fn test_cli_init_preserves_config_structure() {
    let test_dir = setup_test_dir("init_structure");
    let config_path = test_dir.join("config.toml");

    let binary = get_binary_path();
    let output = Command::new(&binary)
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .output()
        .expect("Failed to execute init");

    assert!(output.status.success());

    let content = fs::read_to_string(&config_path).expect("Failed to read config");

    // Verify all required sections exist
    assert!(content.contains("[sqllog]"));
    assert!(content.contains("[error]"));
    assert!(content.contains("[logging]"));
    assert!(content.contains("directory"));
    assert!(content.contains("level"));
    assert!(content.contains("retention_days"));
}

#[test]
fn test_cli_validate_success_returns_zero() {
    let test_dir = setup_test_dir("validate_success");
    let config_path = test_dir.join("config.toml");

    let binary = get_binary_path();

    let init_output = Command::new(&binary)
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .output()
        .expect("Failed to execute init");

    assert!(init_output.status.success());

    let validate_output = Command::new(&binary)
        .arg("validate")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute validate");

    assert!(
        validate_output.status.success(),
        "Valid config should validate"
    );
    assert!(validate_output.status.code() == Some(0));
}

#[test]
fn test_cli_run_with_large_log_output() {
    let test_dir = setup_test_env("run_large_output");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");

    // Create larger log file
    let log_file = sqllog_dir.join("large.log");
    let mut content = String::new();
    for i in 0..100 {
        let exec_id = 257_809_109 + i;
        use std::fmt::Write;
        writeln!(
            &mut content,
            "2025-10-20 15:10:{:02}.614 (EP[0] sess:0x{:x} thrd:2188515 user:OASIS_MSG trxid:0 stmt:0x7f41435677a8 appname: ip:::ffff:10.63.97.88) [INS] INSERT INTO OASIS_MSG.SYS_NOTIFY_TODOTARGET VALUES( ?,?,? ) EXECTIME: 3(ms) ROWCOUNT: 1(rows) EXEC_ID: {exec_id}.",
            i % 60,
            i
        ).expect("Failed to write to string");
    }
    fs::write(&log_file, content).expect("Failed to write log file");

    let binary = get_binary_path();

    let init_output = Command::new(&binary)
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .output()
        .expect("Failed to execute init");

    assert!(init_output.status.success());

    let mut config = fs::read_to_string(&config_path).expect("Failed to read config");
    let sqllog_display = sqllog_dir.to_string_lossy().to_string().replace('\\', "/");
    config = config.replace("sqllogs", &sqllog_display);
    fs::write(&config_path, config).expect("Failed to write config");

    let run_output = Command::new(&binary)
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute run");

    assert!(run_output.status.success());
}

fn setup_test_env(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_e2e_extra").join(name);
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    test_dir
}

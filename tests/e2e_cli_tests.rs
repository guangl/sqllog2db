//! 端到端 CLI 集成测试 - 简化版
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn setup_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_e2e_outputs").join(name);
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
";
    fs::write(log_file, content).expect("Failed to write log file");
}

#[test]
fn test_cli_run_end_to_end() {
    let test_dir = setup_test_dir("e2e_run");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");
    let log_file = sqllog_dir.join("sample.log");
    create_sample_log(&log_file);

    let binary = get_binary_path();

    // Step 1: Generate config
    let init_output = Command::new(&binary)
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .output()
        .expect("Failed to execute init");

    assert!(init_output.status.success());
    assert!(config_path.exists());

    // Step 2: Update config for test
    let mut config = fs::read_to_string(&config_path).expect("Failed to read config");
    let sqllog_display = sqllog_dir.to_string_lossy().to_string().replace('\\', "/");
    config = config.replace("sqllogs", &sqllog_display);
    fs::write(&config_path, config).expect("Failed to write updated config");

    // Step 3: Validate
    let validate_output = Command::new(&binary)
        .arg("validate")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute validate");

    assert!(validate_output.status.success());

    // Step 4: Run
    let run_output = Command::new(&binary)
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute run");

    assert!(run_output.status.success());
}

#[test]
fn test_cli_run_no_logs() {
    let test_dir = setup_test_dir("e2e_no_logs");
    let config_path = test_dir.join("config.toml");
    let sqllog_dir = test_dir.join("sqllogs");

    fs::create_dir_all(&sqllog_dir).expect("Failed to create sqllog dir");

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
    fs::write(&config_path, config).expect("Failed to write updated config");

    let run_output = Command::new(&binary)
        .arg("run")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute run");

    assert!(run_output.status.success());
}

#[test]
fn test_cli_run_verbose() {
    let test_dir = setup_test_dir("e2e_verbose");
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
}

#[test]
fn test_cli_run_quiet() {
    let test_dir = setup_test_dir("e2e_quiet");
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
        .arg("--quiet")
        .output()
        .expect("Failed to execute run");

    assert!(run_output.status.success());
}

//! 真正的 CLI 集成测试 - 实际执行二进制并测试 CLI 命令
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn setup_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_integration_outputs").join(name);
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

#[test]
fn test_cli_init_command() {
    let test_dir = setup_test_dir("init_test");
    let config_path = test_dir.join("config.toml");

    let binary = get_binary_path();
    let output = Command::new(&binary)
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .output()
        .expect("Failed to execute init command");

    assert!(
        output.status.success(),
        "init command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(config_path.exists(), "Config file should be created");
}

#[test]
fn test_cli_version() {
    let binary = get_binary_path();
    let output = Command::new(&binary)
        .arg("--version")
        .output()
        .expect("Failed to execute version command");

    assert!(output.status.success(), "version command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.is_empty(),
        "version output should contain version info"
    );
}

#[test]
fn test_cli_help() {
    let binary = get_binary_path();
    let output = Command::new(&binary)
        .arg("--help")
        .output()
        .expect("Failed to execute help command");

    assert!(output.status.success(), "help command failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "help should contain output");
}

#[test]
fn test_cli_invalid_command() {
    let binary = get_binary_path();
    let output = Command::new(&binary)
        .arg("invalid_command_xyz")
        .output()
        .expect("Failed to execute invalid command");

    assert!(!output.status.success(), "invalid command should fail");
}

#[test]
fn test_cli_init_then_validate() {
    let test_dir = setup_test_dir("init_validate_test");
    let config_path = test_dir.join("config.toml");

    let binary = get_binary_path();

    // Step 1: Init
    let init_output = Command::new(&binary)
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .output()
        .expect("Failed to execute init");

    assert!(init_output.status.success(), "init should succeed");
    assert!(config_path.exists(), "config should be created");

    // Step 2: Validate
    let validate_output = Command::new(&binary)
        .arg("validate")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute validate");

    assert!(validate_output.status.success(), "validate should succeed");
}

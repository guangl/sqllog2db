use std::path::PathBuf;
use std::process::Command;

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
fn test_cli_self_update_help() {
    let binary = get_binary_path();
    let output = Command::new(&binary)
        .arg("self-update")
        .arg("--help")
        .output()
        .expect("Failed to execute self-update --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Self-update the application to the latest version"));
    assert!(stdout.contains("--check"));
}

#[test]
fn test_cli_self_update_check() {
    let binary = get_binary_path();
    // This might fail if there's no internet or no releases, but we want to see it running
    let output = Command::new(&binary)
        .arg("self-update")
        .arg("--check")
        .output()
        .expect("Failed to execute self-update --check");

    // We don't necessarily expect success if there's no internet,
    // but we want to ensure the command is recognized.
    // If it's NOT recognized, clap will exit with error and help message.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // If the command was recognized, it should print "Current version"
    let combined_output = format!("{stdout}{stderr}");
    assert!(combined_output.contains("Current version:"));
}

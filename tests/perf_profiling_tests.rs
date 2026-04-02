#[cfg(feature = "filters")]
use dm_database_sqllog2db::cli::run::handle_run;
#[cfg(feature = "filters")]
use dm_database_sqllog2db::config::Config;
#[cfg(feature = "filters")]
use std::fs;
#[cfg(feature = "filters")]
use std::path::PathBuf;

#[cfg(feature = "filters")]
#[test]
fn test_perf_profiling() {
    let test_dir = PathBuf::from("target/test_perf_profiling");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let sqllog_dir = test_dir.join("sqllogs");
    fs::create_dir_all(&sqllog_dir).unwrap();

    // Create some dummy logs
    use std::fmt::Write as _;
    let mut logs = String::new();
    for i in 0..5000 {
        let _ = writeln!(
            logs,
            "2025-10-20 15:10:28.615 (EP[0] sess:0x1 user:U1 trxid:{i} stmt:0x2 appname: ip:1.1.1.1) [INS] INSERT DATA {i}. EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: {i}."
        );
    }
    fs::write(sqllog_dir.join("test.log"), logs).unwrap();

    let config_toml = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}/errors.log"

[logging]
file = "{}/app.log"
level = "info"
retention_days = 7

[features.filters]
enable = false

[exporter.csv]
file = "{}/output.csv"
overwrite = true
append = false
"#,
        sqllog_dir.to_string_lossy().replace('\\', "/"),
        test_dir.to_string_lossy().replace('\\', "/"),
        test_dir.to_string_lossy().replace('\\', "/"),
        test_dir.to_string_lossy().replace('\\', "/")
    );

    let config: Config = toml::from_str(&config_toml).unwrap();

    println!("--- Starting handle_run with enable=false ---");
    handle_run(&config).unwrap();
    println!("--- Finished handle_run with enable=false ---");

    let mut config_enabled = config.clone();
    if let Some(f) = &mut config_enabled.features.filters {
        f.enable = true;
        // Add a filter that matches everything to compare overhead
        f.meta.start_ts = Some("2000-01-01".to_string());
    }

    println!("--- Starting handle_run with enable=true ---");
    handle_run(&config_enabled).unwrap();
    println!("--- Finished handle_run with enable=true ---");
}

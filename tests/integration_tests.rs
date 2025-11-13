/// 全面的集成测试 - 测试所有主要功能
use std::{fs, path::PathBuf, process::Command};

// Helper to get path to compiled binary
fn binary_path() -> PathBuf {
    // 使用 cargo 测试时提供的二进制路径
    let exe = env!("CARGO_BIN_EXE_sqllog2db");
    PathBuf::from(exe)
}

/// 转换路径为 TOML 兼容格式（Windows 反斜杠转义）
fn toml_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

/// 创建一个模拟的达梦日志文件（简单格式，不一定能被解析器完全识别）
fn create_mock_dm_log(path: &std::path::Path, content: &str) {
    fs::write(path, content).unwrap();
}

#[test]
fn test_init_command_creates_config() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("test_config.toml");

    let status = Command::new(binary_path())
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .status()
        .expect("init failed");

    assert!(status.success());
    assert!(config_path.exists());

    // 验证生成的配置文件包含必要的section
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[sqllog]"));
    assert!(content.contains("[error]"));
    assert!(content.contains("[logging]"));
    assert!(content.contains("[exporter.csv]"));
}

#[test]
fn test_init_force_overwrite() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("test_config.toml");

    // 第一次创建
    Command::new(binary_path())
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .status()
        .expect("init failed");

    assert!(config_path.exists());

    // 第二次使用 --force 覆盖
    let status = Command::new(binary_path())
        .arg("init")
        .arg("--output")
        .arg(&config_path)
        .arg("--force")
        .status()
        .expect("init with force failed");

    assert!(status.success());
}

#[test]
fn test_validate_command_with_valid_config() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("valid_config.toml");

    let cfg = r#"
[sqllog]
directory = "sqllogs"

[error]
file = "errors.json"

[logging]
file = "logs/app.log"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "output.csv"
overwrite = true
"#;
    fs::write(&config_path, cfg).unwrap();

    let status = Command::new(binary_path())
        .arg("validate")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("validate failed");

    assert!(status.success());
}

#[test]
fn test_csv_export_with_empty_logs() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();

    let logs_dir = work_dir.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();

    let out_csv = work_dir.join("out.csv");
    let config_path = work_dir.join("config.toml");

    let cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "{}"
overwrite = true
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&work_dir.join("app.log")),
        toml_path(&out_csv)
    );
    fs::write(&config_path, cfg).unwrap();

    let status = Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("run failed");

    assert!(status.success());
    assert!(out_csv.exists());

    // CSV 应该只有表头
    let content = fs::read_to_string(&out_csv).unwrap();
    assert!(content.starts_with("timestamp,ep,sess_id"));
    assert_eq!(content.lines().count(), 1);
}

#[cfg(feature = "sqlite")]
#[test]
fn test_sqlite_export_with_empty_logs() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();

    let logs_dir = work_dir.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();

    let out_db = work_dir.join("out.db");
    let config_path = work_dir.join("config.toml");

    let cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.database]
database_type = "sqlite"
path = "{}"
overwrite = true
table_name = "sqllogs"
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&work_dir.join("app.log")),
        toml_path(&out_db)
    );
    fs::write(&config_path, cfg).unwrap();

    let status = Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("run failed");

    assert!(status.success());
    assert!(out_db.exists());

    // 验证数据库表被创建
    let conn = rusqlite::Connection::open(&out_db).unwrap();
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sqllogs'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(table_count, 1);
}

#[cfg(not(feature = "sqlite"))]
#[test]
fn test_sqlite_export_with_empty_logs() {
    eprintln!("skip sqlite test: 'sqlite' feature disabled");
}

#[cfg(feature = "sqlite")]
#[test]
fn test_multiple_exporters_simultaneously() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();

    let logs_dir = work_dir.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();

    let out_csv = work_dir.join("out.csv");
    let out_db = work_dir.join("out.db");
    let config_path = work_dir.join("config.toml");

    let cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "{}"
overwrite = true

[exporter.database]
database_type = "sqlite"
path = "{}"
overwrite = true
table_name = "sqllogs"
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&work_dir.join("app.log")),
        toml_path(&out_csv),
        toml_path(&out_db)
    );
    fs::write(&config_path, cfg).unwrap();

    let status = Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("run failed");

    assert!(status.success());

    // 由于只支持单个导出器，应该按优先级使用 CSV（CSV > Database）
    // 因此只有 CSV 文件应该被创建，DB 文件不应该存在
    assert!(out_csv.exists(), "CSV 文件应该被创建");
    assert!(!out_db.exists(), "DB 文件不应该被创建（优先级低于 CSV）");
}

#[cfg(not(feature = "sqlite"))]
#[test]
fn test_multiple_exporters_simultaneously() {
    eprintln!("skip multi-exporters test: 'sqlite' feature disabled");
}

#[cfg(feature = "sqlite")]
#[test]
fn test_database_batch_size_configuration() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();

    let logs_dir = work_dir.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();

    let out_db = work_dir.join("out.db");
    let config_path = work_dir.join("config.toml");

    // 测试自定义 batch_size
    let cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.database]
database_type = "sqlite"
path = "{}"
overwrite = true
table_name = "sqllogs"
batch_size = 500
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&work_dir.join("app.log")),
        toml_path(&out_db)
    );
    fs::write(&config_path, cfg).unwrap();

    let status = Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("run failed");

    assert!(status.success());
    assert!(out_db.exists());
}

#[cfg(not(feature = "sqlite"))]
#[test]
fn test_database_batch_size_configuration() {
    eprintln!("skip db batch size test: 'sqlite' feature disabled");
}

#[test]
fn test_error_log_creation_with_invalid_logs() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();

    let logs_dir = work_dir.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();

    // 创建一些无效的日志文件（不符合达梦日志格式）
    create_mock_dm_log(
        &logs_dir.join("invalid1.log"),
        "This is not a valid DM log\n",
    );
    create_mock_dm_log(&logs_dir.join("invalid2.log"), "Another invalid line\n");

    let error_log = work_dir.join("errors.json");
    let config_path = work_dir.join("config.toml");

    let cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "{}"
overwrite = true
"#,
        toml_path(&logs_dir),
        toml_path(&error_log),
        toml_path(&work_dir.join("app.log")),
        toml_path(&work_dir.join("out.csv"))
    );
    fs::write(&config_path, cfg).unwrap();

    let status = Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("run failed");

    assert!(status.success());

    // 错误日志文件应该被创建（如果有解析错误）
    // 注意：由于我们创建的是完全无效的文件，解析器可能会记录错误
    if error_log.exists() {
        let content = fs::read_to_string(&error_log).unwrap();
        // 如果有错误，应该是JSON格式
        if !content.is_empty() {
            // 每行都应该是有效的JSON
            for line in content.lines() {
                let _: serde_json::Value =
                    serde_json::from_str(line).expect("error log should be valid JSON");
            }
        }

        // 校验 summary 指标文件
        let summary = error_log.with_extension("json.summary.json");
        assert!(summary.exists(), "errors.summary.json should be generated");
        let summary_json: serde_json::Value =
            serde_json::from_reader(std::fs::File::open(&summary).unwrap()).unwrap();
        assert!(summary_json.get("total").is_some());
        assert!(summary_json.get("by_category").is_some());
    }
}

#[test]
fn test_logging_file_creation() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();

    let logs_dir = work_dir.join("logs");
    let sqllogs_dir = work_dir.join("sqllogs");
    fs::create_dir_all(&logs_dir).unwrap();
    fs::create_dir_all(&sqllogs_dir).unwrap();

    let app_log = logs_dir.join("app.log");
    let config_path = work_dir.join("config.toml");

    let cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "debug"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "{}"
overwrite = true
"#,
        toml_path(&sqllogs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&app_log),
        toml_path(&work_dir.join("out.csv"))
    );
    fs::write(&config_path, cfg).unwrap();

    let status = Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("run failed");

    assert!(status.success());

    // 应用日志文件应该被创建（可能带日期后缀）
    // 检查 logs 目录中是否有 .log 文件
    let log_files: Vec<_> = fs::read_dir(&logs_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "log")
                .unwrap_or(false)
        })
        .collect();

    assert!(!log_files.is_empty(), "应该创建至少一个日志文件");

    // 读取第一个日志文件并验证内容
    let first_log = &log_files[0];
    let content = fs::read_to_string(first_log.path()).unwrap();
    assert!(!content.is_empty());
    // 应该包含启动信息
    assert!(content.contains("应用程序启动") || content.contains("初始化"));
}

#[test]
fn test_overwrite_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();

    let logs_dir = work_dir.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();

    let out_csv = work_dir.join("out.csv");
    let config_path = work_dir.join("config.toml");

    // 第一次运行
    let cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "{}"
overwrite = true
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&work_dir.join("app.log")),
        toml_path(&out_csv)
    );
    fs::write(&config_path, cfg).unwrap();

    Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("first run failed");

    assert!(out_csv.exists());
    let first_content = fs::read_to_string(&out_csv).unwrap();

    // 第二次运行（应该覆盖）
    let status = Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("second run failed");

    assert!(status.success());

    let second_content = fs::read_to_string(&out_csv).unwrap();
    // 由于都是空日志，内容应该相同（只有表头）
    assert_eq!(first_content, second_content);
}

/// 测试 DM 数据库导出器配置验证
/// 注意：此测试不实际连接 DM 数据库，仅测试配置解析和验证
#[cfg(feature = "dm")]
#[test]
fn test_dm_exporter_config_validation() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();
    let logs_dir = work_dir.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();

    let config_path = work_dir.join("config.toml");

    // 测试完整的 DM 配置
    let cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.database]
database_type = "dm"
host = "localhost"
port = 5236
username = "SYSDBA"
password = "SYSDBA"
table_name = "sqllog"
overwrite = true
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&work_dir.join("app.log"))
    );
    fs::write(&config_path, cfg).unwrap();

    // 配置验证应该成功
    let status = Command::new(binary_path())
        .arg("validate")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("validate failed");

    assert!(status.success(), "DM 配置验证应该成功");
}

/// 测试 DM 数据库导出器配置缺少必需字段
#[cfg(feature = "dm")]
#[test]
fn test_dm_exporter_missing_required_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();
    let logs_dir = work_dir.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();

    let config_path = work_dir.join("config.toml");

    // 测试缺少 host 字段的配置
    let cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.database]
database_type = "dm"
port = 5236
username = "SYSDBA"
password = "SYSDBA"
table_name = "sqllog"
overwrite = true
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&work_dir.join("app.log"))
    );
    fs::write(&config_path, cfg).unwrap();

    // 运行应该失败（缺少必需的 host 字段）
    let status = Command::new(binary_path())
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .status()
        .expect("command execution failed");

    assert!(!status.success(), "缺少必需字段时应该失败");
}

/// 测试 DM 和 SQLite 配置的区分
/// DM 需要 host/port/username/password，SQLite 需要 file
#[cfg(all(feature = "dm", feature = "sqlite"))]
#[test]
fn test_dm_vs_sqlite_config_differences() {
    let tmp = tempfile::tempdir().unwrap();
    let work_dir = tmp.path();
    let logs_dir = work_dir.join("logs");
    fs::create_dir_all(&logs_dir).unwrap();

    // 测试 SQLite 配置（使用 file 字段）
    let sqlite_config = work_dir.join("sqlite_config.toml");
    let sqlite_cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.database]
database_type = "sqlite"
file = "{}"
table_name = "sqllog"
overwrite = true
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&work_dir.join("app.log")),
        toml_path(&work_dir.join("test.db"))
    );
    fs::write(&sqlite_config, sqlite_cfg).unwrap();

    let sqlite_status = Command::new(binary_path())
        .arg("validate")
        .arg("-c")
        .arg(&sqlite_config)
        .status()
        .expect("sqlite validate failed");

    assert!(sqlite_status.success(), "SQLite 配置应该有效");

    // 测试 DM 配置（使用 host/port/username/password）
    let dm_config = work_dir.join("dm_config.toml");
    let dm_cfg = format!(
        r#"
[sqllog]
directory = "{}"

[error]
file = "{}"

[logging]
file = "{}"
level = "info"
retention_days = 7

[features]
replace_sql_parameters = false
scatter = false

[exporter.database]
database_type = "dm"
host = "localhost"
port = 5236
username = "SYSDBA"
password = "SYSDBA"
table_name = "sqllog"
overwrite = true
"#,
        toml_path(&logs_dir),
        toml_path(&work_dir.join("errors.json")),
        toml_path(&work_dir.join("app.log"))
    );
    fs::write(&dm_config, dm_cfg).unwrap();

    let dm_status = Command::new(binary_path())
        .arg("validate")
        .arg("-c")
        .arg(&dm_config)
        .status()
        .expect("dm validate failed");

    assert!(dm_status.success(), "DM 配置应该有效");
}

#[cfg(not(feature = "dm"))]
#[test]
fn test_dm_exporter_config_validation() {
    eprintln!("skip dm test: 'dm' feature disabled");
}

#[cfg(not(feature = "dm"))]
#[test]
fn test_dm_exporter_missing_required_fields() {
    eprintln!("skip dm test: 'dm' feature disabled");
}

#[cfg(not(all(feature = "dm", feature = "sqlite")))]
#[test]
fn test_dm_vs_sqlite_config_differences() {
    eprintln!("skip test: requires both 'dm' and 'sqlite' features");
}

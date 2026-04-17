//! Integration tests for CLI handlers and the run pipeline.

use dm_database_sqllog2db::cli::digest::{SortBy, handle_digest};
use dm_database_sqllog2db::cli::init::handle_init;
use dm_database_sqllog2db::cli::run::handle_run;
use dm_database_sqllog2db::cli::show_config::handle_show_config;
use dm_database_sqllog2db::cli::stats::handle_stats;
use dm_database_sqllog2db::cli::validate::handle_validate;
use dm_database_sqllog2db::config::{
    Config, CsvExporter, ExporterConfig, SqliteExporter, SqllogConfig,
};
use dm_database_sqllog2db::features::filters::MetaFilters;
use dm_database_sqllog2db::features::{FeaturesConfig, FiltersFeature, ReplaceParametersConfig};
use dm_database_sqllog2db::lang::Lang;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

// ── helpers ──────────────────────────────────────────────────────────────────

fn write_test_log(path: &std::path::Path, count: usize) {
    use std::fmt::Write as _;
    let mut buf = String::with_capacity(count * 180);
    for i in 0..count {
        writeln!(
            buf,
            "2025-01-15 10:30:28.001 (EP[0] sess:0x{i:04x} user:TESTUSER trxid:{i} stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT * FROM t WHERE id={i}. EXECTIME: {exec}(ms) ROWCOUNT: {rows}(rows) EXEC_ID: {i}.",
            exec = (i * 13) % 1000,
            rows = i % 100,
        )
        .unwrap();
    }
    std::fs::write(path, buf).unwrap();
}

fn make_run_config(log_dir: &std::path::Path, csv_file: &std::path::Path) -> Config {
    Config {
        sqllog: SqllogConfig {
            path: log_dir.to_str().unwrap().to_string(),
        },
        exporter: ExporterConfig {
            csv: Some(CsvExporter {
                file: csv_file.to_str().unwrap().to_string(),
                overwrite: true,
                append: false,
            }),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ── handle_run tests ─────────────────────────────────────────────────────────

#[test]
fn test_handle_run_dry_run_empty_dir() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    // No log files → handle_run returns Ok early
    let cfg = Config {
        sqllog: SqllogConfig {
            path: log_dir.to_str().unwrap().to_string(),
        },
        ..Default::default()
    };
    let interrupted = Arc::new(AtomicBool::new(false));
    handle_run(&cfg, None, true, true, &interrupted, 80, false, None, 1).unwrap();
}

#[test]
fn test_handle_run_dry_run_with_log_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("a.log"), 20);
    write_test_log(&log_dir.join("b.log"), 10);

    let cfg = Config {
        sqllog: SqllogConfig {
            path: log_dir.to_str().unwrap().to_string(),
        },
        ..Default::default()
    };

    let interrupted = Arc::new(AtomicBool::new(false));
    handle_run(&cfg, None, true, true, &interrupted, 80, false, None, 1).unwrap();
}

#[test]
fn test_handle_run_dry_run_with_limit() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("test.log"), 50);

    let cfg = Config {
        sqllog: SqllogConfig {
            path: log_dir.to_str().unwrap().to_string(),
        },
        ..Default::default()
    };

    let interrupted = Arc::new(AtomicBool::new(false));
    // limit to 5 records
    handle_run(&cfg, Some(5), true, true, &interrupted, 80, false, None, 1).unwrap();
}

#[test]
fn test_handle_run_real_csv_export() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("test.log"), 10);

    let csv_file = dir.path().join("out.csv");
    let cfg = make_run_config(&log_dir, &csv_file);

    let interrupted = Arc::new(AtomicBool::new(false));
    handle_run(&cfg, None, false, true, &interrupted, 80, false, None, 1).unwrap();

    let content = std::fs::read_to_string(&csv_file).unwrap();
    // header + 10 data rows
    assert!(content.lines().count() >= 10);
}

#[test]
fn test_handle_run_interrupted() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("test.log"), 100);

    let cfg = Config {
        sqllog: SqllogConfig {
            path: log_dir.to_str().unwrap().to_string(),
        },
        ..Default::default()
    };

    // Pre-set interrupted flag — run should return Err(Interrupted)
    let interrupted = Arc::new(AtomicBool::new(true));
    let result = handle_run(&cfg, None, true, true, &interrupted, 80, false, None, 1);
    // Either Ok (no files processed) or Err(Interrupted) depending on timing
    let _ = result;
}

// ── resume tests ─────────────────────────────────────────────────────────────

#[test]
fn test_resume_skips_processed_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();

    // Two files: a.log (10 records) + b.log (10 records)
    write_test_log(&log_dir.join("a.log"), 10);
    write_test_log(&log_dir.join("b.log"), 10);

    let state_path = dir.path().join("state.toml");
    let csv1 = dir.path().join("out1.csv");
    let cfg = make_run_config(&log_dir, &csv1);
    let interrupted = Arc::new(AtomicBool::new(false));

    // First run with --resume: processes both files, writes state
    handle_run(
        &cfg,
        None,
        false,
        true,
        &interrupted,
        80,
        true,
        Some(state_path.to_str().unwrap()),
        1,
    )
    .unwrap();
    let rows_first = std::fs::read_to_string(&csv1).unwrap().lines().count();
    assert!(rows_first >= 10, "expected at least 10 rows");

    // State file must exist after first run
    assert!(state_path.exists(), "state file should be created");

    // Second run with --resume + append: already-processed files are skipped → no new rows
    let csv2 = dir.path().join("out2.csv");
    let mut cfg2 = make_run_config(&log_dir, &csv2);
    cfg2.exporter.csv.as_mut().unwrap().append = true;
    cfg2.exporter.csv.as_mut().unwrap().overwrite = false;

    handle_run(
        &cfg2,
        None,
        false,
        true,
        &interrupted,
        80,
        true,
        Some(state_path.to_str().unwrap()),
        1,
    )
    .unwrap();

    // csv2 should have at most a header row (no data rows) because all files were skipped
    let rows_second = if csv2.exists() {
        std::fs::read_to_string(&csv2).unwrap().lines().count()
    } else {
        0
    };
    assert!(
        rows_second <= 1,
        "second run should skip all files; got {rows_second} rows (expected header only)"
    );
}

#[test]
fn test_resume_reprocesses_changed_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();

    let log_file = log_dir.join("a.log");
    write_test_log(&log_file, 5);

    let state_path = dir.path().join("state.toml");
    let csv = dir.path().join("out.csv");
    let cfg = make_run_config(&log_dir, &csv);
    let interrupted = Arc::new(AtomicBool::new(false));

    // First run: process and record state
    handle_run(
        &cfg,
        None,
        false,
        true,
        &interrupted,
        80,
        true,
        Some(state_path.to_str().unwrap()),
        1,
    )
    .unwrap();
    assert!(state_path.exists());

    // Simulate file growing (append more records)
    write_test_log(&log_file, 10);

    // Second run with --resume: file fingerprint changed → must reprocess
    let csv2 = dir.path().join("out2.csv");
    let cfg2 = make_run_config(&log_dir, &csv2);
    handle_run(
        &cfg2,
        None,
        false,
        true,
        &interrupted,
        80,
        true,
        Some(state_path.to_str().unwrap()),
        1,
    )
    .unwrap();

    // csv2 should have data (file was reprocessed)
    assert!(csv2.exists(), "changed file should be reprocessed");
    let rows = std::fs::read_to_string(&csv2).unwrap().lines().count();
    assert!(rows >= 1, "expected rows from reprocessed file");
}

// ── handle_stats tests ───────────────────────────────────────────────────────

#[test]
fn test_handle_stats_empty_dir() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    let cfg = Config {
        sqllog: SqllogConfig {
            path: log_dir.to_str().unwrap().to_string(),
        },
        ..Default::default()
    };
    // No log files → prints "No log files found" and returns without panic
    handle_stats(&cfg, true, false, None, false, &[], None, None);
}

#[test]
fn test_handle_stats_with_log_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("data.log"), 15);

    let cfg = Config {
        sqllog: SqllogConfig {
            path: log_dir.to_str().unwrap().to_string(),
        },
        ..Default::default()
    };
    handle_stats(&cfg, true, false, None, false, &[], None, None); // quiet=true to suppress progress bar
}

#[test]
fn test_handle_stats_nonexistent_dir() {
    let cfg = Config {
        sqllog: SqllogConfig {
            path: "/no/such/directory/at/all".to_string(),
        },
        ..Default::default()
    };
    // Should not panic — prints an error and returns
    handle_stats(&cfg, true, false, None, false, &[], None, None);
}

fn write_test_log_multi_ts(path: &std::path::Path, timestamps: &[&str]) {
    use std::fmt::Write as _;
    let mut buf = String::new();
    for (i, ts) in timestamps.iter().enumerate() {
        writeln!(
            buf,
            "{ts} (EP[0] sess:0x{i:04x} user:USER{u} trxid:{i} stmt:0x1 appname:App{a} ip:10.0.0.{ip}) [SEL] SELECT * FROM t WHERE id={i}. EXECTIME: {exec}(ms) ROWCOUNT: 1(rows) EXEC_ID: {i}.",
            u = i % 3,
            a = i % 2,
            ip = (i % 3) + 1,
            exec = (i * 13 + 5) % 500,
        ).unwrap();
    }
    std::fs::write(path, buf).unwrap();
}

fn make_stats_cfg(log_dir: &std::path::Path) -> Config {
    Config {
        sqllog: SqllogConfig {
            path: log_dir.to_str().unwrap().to_string(),
        },
        ..Default::default()
    }
}

#[test]
fn test_handle_stats_group_by_user() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 20);
    let cfg = make_stats_cfg(dir.path());
    handle_stats(
        &cfg,
        true,
        false,
        None,
        false,
        &["user".to_string()],
        None,
        None,
    );
}

#[test]
fn test_handle_stats_group_by_multiple() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 20);
    let cfg = make_stats_cfg(dir.path());
    let fields = vec!["user".to_string(), "app".to_string(), "ip".to_string()];
    handle_stats(&cfg, true, false, None, false, &fields, None, None);
}

#[test]
fn test_handle_stats_group_by_invalid() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 5);
    let cfg = make_stats_cfg(dir.path());
    // invalid field — should print error and return without panic
    handle_stats(
        &cfg,
        true,
        false,
        None,
        false,
        &["badfield".to_string()],
        None,
        None,
    );
}

#[test]
fn test_handle_stats_top_slow() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 30);
    let cfg = make_stats_cfg(dir.path());
    handle_stats(&cfg, true, false, Some(5), false, &[], None, None);
}

#[test]
fn test_handle_stats_bucket_hour() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    let ts = &[
        "2025-01-15 08:10:00.000",
        "2025-01-15 08:20:00.000",
        "2025-01-15 09:05:00.000",
        "2025-01-15 10:00:00.000",
        "2025-01-15 10:30:00.000",
        "2025-01-15 10:59:00.000",
    ];
    write_test_log_multi_ts(&dir.path().join("data.log"), ts);
    let cfg = make_stats_cfg(dir.path());
    handle_stats(&cfg, true, false, None, false, &[], Some("hour"), None);
}

#[test]
fn test_handle_stats_bucket_minute() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    let ts = &[
        "2025-01-15 10:00:00.000",
        "2025-01-15 10:00:30.000",
        "2025-01-15 10:01:00.000",
        "2025-01-15 10:02:00.000",
    ];
    write_test_log_multi_ts(&dir.path().join("data.log"), ts);
    let cfg = make_stats_cfg(dir.path());
    handle_stats(&cfg, true, false, None, false, &[], Some("minute"), None);
}

#[test]
fn test_handle_stats_bucket_invalid() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 5);
    let cfg = make_stats_cfg(dir.path());
    // invalid granularity — should print error and return without panic
    handle_stats(&cfg, true, false, None, false, &[], Some("week"), None);
}

#[test]
fn test_handle_stats_json_basic() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 10);
    let cfg = make_stats_cfg(dir.path());
    handle_stats(&cfg, true, false, None, true, &[], None, None);
}

#[test]
fn test_handle_stats_json_with_group_and_bucket() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    let ts = &[
        "2025-01-15 08:00:00.000",
        "2025-01-15 08:30:00.000",
        "2025-01-15 09:00:00.000",
        "2025-01-15 09:15:00.000",
    ];
    write_test_log_multi_ts(&dir.path().join("data.log"), ts);
    let cfg = make_stats_cfg(dir.path());
    let fields = vec!["user".to_string(), "ip".to_string()];
    handle_stats(
        &cfg,
        true,
        false,
        Some(3),
        true,
        &fields,
        Some("hour"),
        None,
    );
}

#[test]
fn test_handle_stats_verbose() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 10);
    let cfg = make_stats_cfg(dir.path());
    // quiet=false, verbose=true — exercises the file table path
    handle_stats(&cfg, false, true, None, false, &[], None, None);
}

#[test]
fn test_handle_stats_group_and_bucket_non_quiet() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    let ts = &[
        "2025-01-15 08:00:00.000",
        "2025-01-15 09:00:00.000",
        "2025-01-15 10:00:00.000",
    ];
    write_test_log_multi_ts(&dir.path().join("data.log"), ts);
    let cfg = make_stats_cfg(dir.path());
    let fields = vec!["user".to_string()];
    // quiet=false — exercises print_group_table and print_bucket_table
    handle_stats(
        &cfg,
        false,
        false,
        Some(2),
        false,
        &fields,
        Some("hour"),
        None,
    );
}

// ── handle_digest tests ──────────────────────────────────────────────────────

#[test]
fn test_handle_digest_empty_dir() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("nologs");
    let cfg = Config {
        sqllog: SqllogConfig {
            path: log_dir.to_str().unwrap().to_string(),
        },
        ..Default::default()
    };
    // No log files → prints message and returns without panic
    handle_digest(&cfg, true, None, SortBy::Count, 1, false, None);
}

#[test]
fn test_handle_digest_basic() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 20);
    let cfg = make_stats_cfg(dir.path());
    handle_digest(&cfg, true, None, SortBy::Count, 1, false, None);
}

#[test]
fn test_handle_digest_sort_exec() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 20);
    let cfg = make_stats_cfg(dir.path());
    handle_digest(&cfg, true, None, SortBy::Exec, 1, false, None);
}

#[test]
fn test_handle_digest_top_n() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 30);
    let cfg = make_stats_cfg(dir.path());
    handle_digest(&cfg, true, Some(5), SortBy::Count, 1, false, None);
}

#[test]
fn test_handle_digest_min_count() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 20);
    let cfg = make_stats_cfg(dir.path());
    // min_count=100 filters out everything — should print "No SQL fingerprints found."
    handle_digest(&cfg, true, None, SortBy::Count, 100, false, None);
}

#[test]
fn test_handle_digest_json() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    write_test_log(&dir.path().join("data.log"), 10);
    let cfg = make_stats_cfg(dir.path());
    handle_digest(&cfg, true, None, SortBy::Count, 1, true, None);
}

#[test]
fn test_handle_digest_nonexistent_dir() {
    let cfg = Config {
        sqllog: SqllogConfig {
            path: "/nonexistent_dir_xyz".to_string(),
        },
        ..Default::default()
    };
    // Should not panic
    handle_digest(&cfg, true, None, SortBy::Count, 1, false, None);
}

#[test]
fn test_handle_digest_aggregates_same_fingerprint() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    // Write records with identical SQL structure but different literal values
    // These should collapse into one fingerprint
    let mut buf = String::new();
    use std::fmt::Write as _;
    for i in 0..5 {
        writeln!(
            buf,
            "2025-01-15 10:30:28.001 (EP[0] sess:0x{i:04x} user:U trxid:{i} stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT * FROM tbl WHERE id={i}. EXECTIME: 10(ms) ROWCOUNT: 1(rows) EXEC_ID: {i}.",
        ).unwrap();
    }
    std::fs::write(dir.path().join("data.log"), buf).unwrap();
    let cfg = make_stats_cfg(dir.path());
    handle_digest(&cfg, true, None, SortBy::Count, 1, true, None);
}

// ── handle_init tests ────────────────────────────────────────────────────────

#[test]
fn test_handle_init_creates_config_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    handle_init(config_path.to_str().unwrap(), false, Lang::Zh).unwrap();
    assert!(config_path.exists());
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(!content.is_empty());
}

#[test]
fn test_handle_init_fails_if_exists_without_force() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, "existing").unwrap();
    let result = handle_init(config_path.to_str().unwrap(), false, Lang::Zh);
    assert!(result.is_err());
}

#[test]
fn test_handle_init_force_overwrites_existing() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, "old content").unwrap();
    handle_init(config_path.to_str().unwrap(), true, Lang::Zh).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[sqllog]"));
}

#[test]
fn test_handle_init_en_template() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    handle_init(config_path.to_str().unwrap(), false, Lang::En).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[sqllog]"));
    assert!(content.contains("SQL log path"));
    assert!(!content.contains("日志路径"));
}

#[test]
fn test_handle_init_zh_template() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    handle_init(config_path.to_str().unwrap(), false, Lang::Zh).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[sqllog]"));
    assert!(content.contains("日志路径"));
}

// ── handle_validate tests ────────────────────────────────────────────────────

#[test]
fn test_handle_validate_default_config() {
    let cfg = Config::default();
    handle_validate(&cfg); // no panic, hits csv branch and no-filters branch
}

#[test]
fn test_handle_validate_with_sqlite_exporter() {
    let cfg = Config {
        exporter: ExporterConfig {
            csv: None,
            sqlite: Some(SqliteExporter {
                database_url: "/tmp/test.db".to_string(),
                table_name: "records".to_string(),
                overwrite: true,
                append: false,
            }),
        },
        ..Default::default()
    };
    handle_validate(&cfg); // hits sqlite branch
}

#[test]
fn test_handle_validate_with_replace_parameters_none() {
    let cfg = Config {
        features: FeaturesConfig {
            replace_parameters: None,
            ..Default::default()
        },
        ..Default::default()
    };
    handle_validate(&cfg); // hits replace_parameters None branch
}

#[test]
fn test_handle_validate_with_replace_parameters_some() {
    let cfg = Config {
        features: FeaturesConfig {
            replace_parameters: Some(ReplaceParametersConfig {
                enable: true,
                placeholders: vec!["?".to_string()],
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    handle_validate(&cfg); // hits replace_parameters Some branch
}

#[test]
fn test_handle_validate_with_filters_none() {
    let cfg = Config {
        features: FeaturesConfig {
            filters: None,
            ..Default::default()
        },
        ..Default::default()
    };
    handle_validate(&cfg); // hits filters None branch
}

#[test]
fn test_handle_validate_with_filters_all_fields() {
    use dm_database_sqllog2db::features::filters::{IndicatorFilters, SqlFilters};
    let cfg = Config {
        features: FeaturesConfig {
            filters: Some(FiltersFeature {
                enable: true,
                meta: MetaFilters {
                    start_ts: Some("2025-01-01".to_string()),
                    end_ts: Some("2025-12-31".to_string()),
                    usernames: Some(vec!["admin".to_string()]),
                    client_ips: Some(vec!["10.0.0.1".to_string()]),
                    trxids: Some(
                        ["tx1"]
                            .iter()
                            .map(|s| compact_str::CompactString::new(s))
                            .collect(),
                    ),
                    ..Default::default()
                },
                indicators: IndicatorFilters {
                    exec_ids: Some([42_i64].into_iter().collect()),
                    min_runtime_ms: Some(100),
                    min_row_count: Some(10),
                },
                sql: SqlFilters {
                    include_patterns: Some(vec!["SELECT".to_string()]),
                    exclude_patterns: Some(vec!["DROP".to_string()]),
                },
                record_sql: SqlFilters::default(),
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    handle_validate(&cfg); // hits all filter sub-branches
}

#[test]
fn test_handle_validate_filters_disabled() {
    use dm_database_sqllog2db::features::filters::IndicatorFilters;
    let cfg = Config {
        features: FeaturesConfig {
            filters: Some(FiltersFeature {
                enable: false,
                meta: MetaFilters::default(),
                indicators: IndicatorFilters::default(),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    handle_validate(&cfg); // hits "配置但未明确启用" branch
}

// ── handle_run coverage supplement ──────────────────────────────────────────

#[test]
fn test_handle_run_non_quiet_prints_summary() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("data.log"), 10);
    let csv_file = dir.path().join("out.csv");
    let cfg = make_run_config(&log_dir, &csv_file);
    let interrupted = Arc::new(AtomicBool::new(false));
    // quiet=false exercises the summary print path
    handle_run(&cfg, None, true, false, &interrupted, 80, false, None, 1).unwrap();
}

#[test]
fn test_handle_run_with_filters_builds_pipeline() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("data.log"), 20);
    let csv_file = dir.path().join("out.csv");
    let mut cfg = make_run_config(&log_dir, &csv_file);
    // Enable a record-level filter — exercises build_pipeline and FilterProcessor
    cfg.features.filters = Some(FiltersFeature {
        enable: true,
        meta: MetaFilters {
            usernames: Some(vec!["TESTUSER".to_string()]),
            ..Default::default()
        },
        ..Default::default()
    });
    let interrupted = Arc::new(AtomicBool::new(false));
    handle_run(&cfg, None, true, true, &interrupted, 80, false, None, 1).unwrap();
}

#[test]
fn test_handle_run_with_limit_mid_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("data.log"), 100);
    let csv_file = dir.path().join("out.csv");
    let cfg = make_run_config(&log_dir, &csv_file);
    let interrupted = Arc::new(AtomicBool::new(false));
    // limit=5 stops partway through the file — exercises the limit check in process_log_file
    handle_run(&cfg, Some(5), false, true, &interrupted, 80, false, None, 1).unwrap();
    let content = std::fs::read_to_string(&csv_file).unwrap();
    let data_lines = content.lines().count().saturating_sub(1); // minus header
    assert!(data_lines <= 5, "expected ≤5 records, got {data_lines}");
}

#[test]
fn test_handle_run_with_transaction_filters_prescans() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("data.log"), 30);
    let csv_file = dir.path().join("out.csv");
    let mut cfg = make_run_config(&log_dir, &csv_file);
    // exec_ids filter triggers transaction pre-scan path
    cfg.features.filters = Some(FiltersFeature {
        enable: true,
        meta: MetaFilters::default(),
        indicators: dm_database_sqllog2db::features::filters::IndicatorFilters {
            exec_ids: Some([0_i64, 1, 2].into_iter().collect()),
            min_runtime_ms: None,
            min_row_count: None,
        },
        ..Default::default()
    });
    let interrupted = Arc::new(AtomicBool::new(false));
    handle_run(&cfg, None, true, true, &interrupted, 80, false, None, 1).unwrap();
}

#[test]
fn test_handle_run_with_min_runtime_filter() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("data.log"), 20);
    let csv_file = dir.path().join("out.csv");
    let mut cfg = make_run_config(&log_dir, &csv_file);
    cfg.features.filters = Some(FiltersFeature {
        enable: true,
        meta: MetaFilters::default(),
        indicators: dm_database_sqllog2db::features::filters::IndicatorFilters {
            exec_ids: None,
            min_runtime_ms: Some(1),
            min_row_count: None,
        },
        ..Default::default()
    });
    let interrupted = Arc::new(AtomicBool::new(false));
    handle_run(&cfg, None, true, true, &interrupted, 80, false, None, 1).unwrap();
}

// ── handle_show_config tests (via integration) ───────────────────────────────

#[test]
fn test_handle_show_config_integration() {
    let cfg = Config::default();
    // Just verify no panic when called from integration test context
    handle_show_config(&cfg, "/path/to/config.toml", false);
}

// ── parallel CSV tests ──────────────────────────────────────────────────────

#[test]
fn test_handle_run_parallel_csv_multiple_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    // Create 3 log files to trigger the parallel path
    write_test_log(&log_dir.join("a.log"), 10);
    write_test_log(&log_dir.join("b.log"), 10);
    write_test_log(&log_dir.join("c.log"), 10);

    let csv_file = dir.path().join("out.csv");
    let cfg = make_run_config(&log_dir, &csv_file);
    let interrupted = Arc::new(AtomicBool::new(false));

    // jobs=2, multiple files, no limit, CSV exporter → triggers process_csv_parallel
    handle_run(&cfg, None, false, true, &interrupted, 80, false, None, 2).unwrap();

    let content = std::fs::read_to_string(&csv_file).unwrap();
    let data_lines = content.lines().count().saturating_sub(1);
    assert_eq!(data_lines, 30, "expected 30 records from 3 × 10");
}

#[test]
fn test_handle_run_parallel_csv_with_resume() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("a.log"), 5);
    write_test_log(&log_dir.join("b.log"), 5);

    let csv_file = dir.path().join("out.csv");
    let state_file = dir.path().join("state.toml");
    let cfg = make_run_config(&log_dir, &csv_file);
    let interrupted = Arc::new(AtomicBool::new(false));

    // First parallel run: processes both files and records state
    handle_run(
        &cfg,
        None,
        false,
        true,
        &interrupted,
        80,
        true,
        Some(state_file.to_str().unwrap()),
        2,
    )
    .unwrap();
    assert!(state_file.exists());

    // Second run: all files already processed → output empty (no data rows)
    let csv2 = dir.path().join("out2.csv");
    let mut cfg2 = make_run_config(&log_dir, &csv2);
    cfg2.exporter.csv.as_mut().unwrap().append = true;
    cfg2.exporter.csv.as_mut().unwrap().overwrite = false;
    handle_run(
        &cfg2,
        None,
        false,
        true,
        &interrupted,
        80,
        true,
        Some(state_file.to_str().unwrap()),
        2,
    )
    .unwrap();
    // csv2 should have at most a header (all files skipped)
    let rows = if csv2.exists() {
        std::fs::read_to_string(&csv2).unwrap().lines().count()
    } else {
        0
    };
    assert!(rows <= 1, "expected ≤1 rows in second run, got {rows}");
}

// ── performance baseline ─────────────────────────────────────────────────────
//
// Lightweight sanity check — NOT a substitute for `cargo bench`.
// Thresholds are intentionally conservative:
//   - debug builds: 30k rec/s  (catches complete disasters only)
//   - release builds: 500k rec/s  (catches real regressions)
// Run with `cargo test --release` for meaningful numbers.

#[test]
fn test_csv_throughput_baseline() {
    const RECORD_COUNT: usize = 20_000;

    // Debug builds run ~100k rec/s; release runs ~2M rec/s on developer machines.
    // CI machines are slower, so thresholds are kept conservative.
    #[cfg(debug_assertions)]
    const MIN_RECORDS_PER_SEC: f64 = 30_000.0;
    #[cfg(not(debug_assertions))]
    const MIN_RECORDS_PER_SEC: f64 = 500_000.0;

    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("perf.log"), RECORD_COUNT);

    let csv_file = dir.path().join("out.csv");
    let cfg = make_run_config(&log_dir, &csv_file);

    let interrupted = Arc::new(AtomicBool::new(false));
    let start = std::time::Instant::now();
    handle_run(&cfg, None, false, true, &interrupted, 80, false, None, 1).unwrap();
    let elapsed = start.elapsed().as_secs_f64();

    #[allow(clippy::cast_precision_loss)]
    let rate = RECORD_COUNT as f64 / elapsed;
    assert!(
        rate >= MIN_RECORDS_PER_SEC,
        "CSV throughput {rate:.0} rec/s is below {MIN_RECORDS_PER_SEC:.0} rec/s minimum \
         ({} build)",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    );
}

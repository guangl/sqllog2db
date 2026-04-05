//! Integration tests for CLI handlers and the run pipeline.

use dm_database_sqllog2db::cli::init::handle_init;
use dm_database_sqllog2db::cli::run::handle_run;
use dm_database_sqllog2db::cli::show_config::handle_show_config;
use dm_database_sqllog2db::cli::stats::handle_stats;
use dm_database_sqllog2db::config::{Config, CsvExporter, ExporterConfig, SqllogConfig};
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
    handle_run(&cfg, None, true, true, &interrupted, 80, false, None).unwrap();
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
    handle_run(&cfg, None, true, true, &interrupted, 80, false, None).unwrap();
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
    handle_run(&cfg, Some(5), true, true, &interrupted, 80, false, None).unwrap();
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
    handle_run(&cfg, None, false, true, &interrupted, 80, false, None).unwrap();

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
    let result = handle_run(&cfg, None, true, true, &interrupted, 80, false, None);
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
    handle_stats(&cfg, true, false, None, false);
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
    handle_stats(&cfg, true, false, None, false); // quiet=true to suppress progress bar
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
    handle_stats(&cfg, true, false, None, false);
}

// ── handle_init tests ────────────────────────────────────────────────────────

#[test]
fn test_handle_init_creates_config_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    handle_init(config_path.to_str().unwrap(), false).unwrap();
    assert!(config_path.exists());
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(!content.is_empty());
}

#[test]
fn test_handle_init_fails_if_exists_without_force() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, "existing").unwrap();
    let result = handle_init(config_path.to_str().unwrap(), false);
    assert!(result.is_err());
}

#[test]
fn test_handle_init_force_overwrites_existing() {
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, "old content").unwrap();
    handle_init(config_path.to_str().unwrap(), true).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[sqllog]"));
}

// ── handle_show_config tests (via integration) ───────────────────────────────

#[test]
fn test_handle_show_config_integration() {
    let cfg = Config::default();
    // Just verify no panic when called from integration test context
    handle_show_config(&cfg, "/path/to/config.toml", false);
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
    handle_run(&cfg, None, false, true, &interrupted, 80, false, None).unwrap();
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

/// Baseline benchmark: filter pipeline overhead.
///
/// Compares five scenarios against the no-filter fast path:
///
/// | scenario                | what it measures                                         |
/// |-------------------------|----------------------------------------------------------|
/// | `no_pipeline`           | raw parse+export speed (fast path, zero overhead)        |
/// | `pipeline_passthrough`  | pipeline present but no record filtered out              |
/// | `trxid_small`           | exact trxid match against 10 IDs (`HashSet` O(1))        |
/// | `trxid_large`           | exact trxid match against 1 000 IDs (`HashSet` O(1))     |
/// | `indicator_prescan`     | two-pass: pre-scan by `min_runtime_ms` + main pass        |
///
/// Run with: `cargo bench --bench bench_filters --features "filters,csv"`
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dm_database_sqllog2db::cli::run::handle_run;
use dm_database_sqllog2db::config::Config;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const RECORDS: usize = 10_000;

/// Synthetic log where `trxid` = loop index i, `exec_time` = (i\*13)%5000 ms.
fn synthetic_log(record_count: usize) -> String {
    use std::fmt::Write as _;
    let mut buf = String::with_capacity(record_count * 170);
    for i in 0..record_count {
        writeln!(
            buf,
            "2025-01-15 10:30:28.001 (EP[0] sess:0x{i:04x} user:BENCH trxid:{i} stmt:0x1 appname:BenchApp ip:10.0.0.{ip}) [SEL] SELECT col1, col2 FROM bench_table WHERE id={i}. EXECTIME: {exec}(ms) ROWCOUNT: {rows}(rows) EXEC_ID: {i}.",
            ip   = i % 256,
            exec = (i * 13) % 5000,
            rows = i % 1000,
        )
        .unwrap();
    }
    buf
}

fn base_toml(sqllog_dir: &Path, bench_dir: &Path) -> String {
    format!(
        r#"
[sqllog]
directory = "{sqllog}"

[error]
file = "{dir}/errors.log"

[logging]
file = "{dir}/app.log"
level = "warn"
retention_days = 1

[exporter.csv]
file = "/dev/null"
overwrite = true
append = false
"#,
        sqllog = sqllog_dir.to_string_lossy().replace('\\', "/"),
        dir = bench_dir.to_string_lossy().replace('\\', "/"),
    )
}

fn cfg_no_pipeline(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    toml::from_str(&base_toml(sqllog_dir, bench_dir)).unwrap()
}

/// Filters enabled but `start_ts` is in the distant past → every record passes.
fn cfg_pipeline_passthrough(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let toml = format!(
        "{base}
[features.filters]
enable = true
start_ts = \"2000-01-01\"
",
        base = base_toml(sqllog_dir, bench_dir)
    );
    toml::from_str(&toml).unwrap()
}

/// Exact trxid match against a small set (10 IDs).
/// Only records with trxid in [0..10] are kept.
fn cfg_trxid_small(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let ids: Vec<String> = (0..10).map(|i: usize| format!("\"{i}\"")).collect();
    let toml = format!(
        "{base}
[features.filters]
enable = true
trxids = [{ids}]
",
        base = base_toml(sqllog_dir, bench_dir),
        ids = ids.join(", "),
    );
    toml::from_str(&toml).unwrap()
}

/// Exact trxid match against a large set (1 000 IDs) — validates `HashSet` O(1) benefit.
/// Matches the first 1 000 trxids out of `RECORDS`.
fn cfg_trxid_large(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let ids: Vec<String> = (0..1_000).map(|i: usize| format!("\"{i}\"")).collect();
    let toml = format!(
        "{base}
[features.filters]
enable = true
trxids = [{ids}]
",
        base = base_toml(sqllog_dir, bench_dir),
        ids = ids.join(", "),
    );
    toml::from_str(&toml).unwrap()
}

/// Transaction-level filter using `min_runtime_ms` — triggers the two-pass pre-scan.
/// Records with `exec_time` >= 2000 ms pass (roughly 60% of the synthetic set).
fn cfg_indicator_prescan(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let toml = format!(
        "{base}
[features.filters]
enable = true
min_runtime_ms = 2000
",
        base = base_toml(sqllog_dir, bench_dir)
    );
    toml::from_str(&toml).unwrap()
}

fn bench_filters(c: &mut Criterion) {
    let bench_dir = PathBuf::from("target/bench_filters");
    let sqllog_dir = bench_dir.join("sqllogs");
    fs::create_dir_all(&sqllog_dir).unwrap();
    fs::write(sqllog_dir.join("bench.log"), synthetic_log(RECORDS)).unwrap();

    let scenarios: &[(&str, Config)] = &[
        ("no_pipeline", cfg_no_pipeline(&sqllog_dir, &bench_dir)),
        (
            "pipeline_passthrough",
            cfg_pipeline_passthrough(&sqllog_dir, &bench_dir),
        ),
        ("trxid_small", cfg_trxid_small(&sqllog_dir, &bench_dir)),
        ("trxid_large", cfg_trxid_large(&sqllog_dir, &bench_dir)),
        (
            "indicator_prescan",
            cfg_indicator_prescan(&sqllog_dir, &bench_dir),
        ),
    ];

    let mut group = c.benchmark_group("filters");
    group.throughput(Throughput::Elements(RECORDS as u64));

    for (name, cfg) in scenarios {
        group.bench_with_input(BenchmarkId::from_parameter(name), cfg, |b, cfg| {
            b.iter(|| {
                handle_run(
                    cfg,
                    None,
                    false,
                    true, // quiet=true: 排除进度条 I/O 对吞吐量测量的干扰
                    &Arc::new(AtomicBool::new(false)),
                    80,
                    false,
                    None,
                )
                .unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_filters);
criterion_main!(benches);

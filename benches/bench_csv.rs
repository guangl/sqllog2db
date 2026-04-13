/// Baseline benchmark: CSV export throughput.
///
/// Measures the full pipeline: log-file parsing → CSV serialization → write to /dev/null.
/// Run with: `cargo bench --bench bench_csv --features csv`
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dm_database_sqllog2db::cli::run::handle_run;
use dm_database_sqllog2db::config::Config;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Build N synthetic `DaMeng` SQL log lines.
fn synthetic_log(record_count: usize) -> String {
    use std::fmt::Write as _;
    let mut buf = String::with_capacity(record_count * 170);
    for i in 0..record_count {
        writeln!(
            buf,
            "2025-01-15 10:30:28.001 (EP[0] sess:0x{i:04x} user:BENCH trxid:{i} stmt:0x1 appname:BenchApp ip:10.0.0.{ip}) [SEL] SELECT col1, col2 FROM bench_table WHERE id={i} AND status='active'. EXECTIME: {exec}(ms) ROWCOUNT: {rows}(rows) EXEC_ID: {i}.",
            ip   = i % 256,
            exec = (i * 13) % 5000,
            rows = i % 1000,
        )
        .unwrap();
    }
    buf
}

fn make_config(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let toml = format!(
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
    );
    toml::from_str(&toml).unwrap()
}

fn bench_csv_export(c: &mut Criterion) {
    let bench_dir = PathBuf::from("target/bench_csv");
    let sqllog_dir = bench_dir.join("sqllogs");
    fs::create_dir_all(&sqllog_dir).unwrap();

    let mut group = c.benchmark_group("csv_export");

    for &n in &[1_000usize, 10_000, 50_000] {
        fs::write(sqllog_dir.join("bench.log"), synthetic_log(n)).unwrap();
        let cfg = make_config(&sqllog_dir, &bench_dir);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &cfg, |b, cfg| {
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
                    1,
                )
                .unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_csv_export);
criterion_main!(benches);

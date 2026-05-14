/// Baseline benchmark: `SQLite` export throughput.
///
/// Uses the aggressive PRAGMA settings already baked into `SqliteExporter`
/// (`JOURNAL_MODE=OFF`, `SYNCHRONOUS=OFF`, `EXCLUSIVE` locking, mmap).
///
/// Run with: `cargo bench --bench bench_sqlite --features sqlite`
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dm_database_sqllog2db::cli::run::handle_run;
use dm_database_sqllog2db::config::Config;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

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

fn make_config(sqllog_dir: &Path, bench_dir: &Path, batch_size: usize) -> Config {
    // Write to a real file — SQLite needs actual block device storage.
    // `overwrite=true` drops+recreates the table on each `handle_run` call,
    // giving a clean slate every benchmark iteration.
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

[exporter.sqlite]
database_url = "{dir}/bench.db"
table_name = "sqllogs"
overwrite = true
append = false
batch_size = {batch_size}
"#,
        sqllog = sqllog_dir.to_string_lossy().replace('\\', "/"),
        dir = bench_dir.to_string_lossy().replace('\\', "/"),
        batch_size = batch_size,
    );
    toml::from_str(&toml).unwrap()
}

fn bench_sqlite_export(c: &mut Criterion) {
    let bench_dir = PathBuf::from("target/bench_sqlite");
    let sqllog_dir = bench_dir.join("sqllogs");
    fs::create_dir_all(&sqllog_dir).unwrap();

    let mut group = c.benchmark_group("sqlite_export");
    // SQLite is slower; fewer iterations keep total time reasonable.
    group.sample_size(20);

    for &n in &[1_000usize, 10_000, 50_000] {
        fs::write(sqllog_dir.join("bench.log"), synthetic_log(n)).unwrap();
        let cfg = make_config(&sqllog_dir, &bench_dir, 10_000);

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
                    None, // compiled_filters
                )
                .unwrap();
            });
        });
    }

    group.finish();
}

fn bench_sqlite_real_file(c: &mut Criterion) {
    let real_dir = PathBuf::from("sqllogs");
    if !real_dir.exists() {
        eprintln!("sqllogs/ not found, skipping sqlite_export_real benchmark");
        return;
    }

    // 独立 bench_dir，避免与 synthetic bench_sqlite 的 bench.db 冲突
    let bench_dir = PathBuf::from("target/bench_sqlite_real");
    fs::create_dir_all(&bench_dir).unwrap();
    let cfg = make_config(&real_dir, &bench_dir, 10_000);

    let mut group = c.benchmark_group("sqlite_export_real");
    // 真实文件 + SQLite 双重慢，尽量减少采样次数（criterion 最小值为 10）
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(120));
    // 记录数未预扫描，省略 Throughput::Elements，仅记录绝对时间
    group.bench_function("real_file", |b| {
        b.iter(|| {
            handle_run(
                &cfg,
                None,
                false,
                true, // quiet=true：排除进度条 I/O
                &Arc::new(AtomicBool::new(false)),
                80,
                false,
                None,
                1,
                None, // compiled_filters
            )
            .unwrap();
        });
    });
    group.finish();
}

fn bench_sqlite_single_row(c: &mut Criterion) {
    let bench_dir = PathBuf::from("target/bench_sqlite_single_row");
    let sqllog_dir = bench_dir.join("sqllogs");
    fs::create_dir_all(&sqllog_dir).unwrap();

    let mut group = c.benchmark_group("sqlite_single_row");
    // 单行提交每次需要 fsync，采样数尽量小以控制总时间
    group.sample_size(10);

    for &n in &[1_000usize, 10_000] {
        fs::write(sqllog_dir.join("bench.log"), synthetic_log(n)).unwrap();
        // batch_size=1 触发每条 INSERT 独立 BEGIN/COMMIT（单行提交对照组）
        let cfg = make_config(&sqllog_dir, &bench_dir, 1);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &cfg, |b, cfg| {
            b.iter(|| {
                handle_run(
                    cfg,
                    None,
                    false,
                    true,
                    &Arc::new(AtomicBool::new(false)),
                    80,
                    false,
                    None,
                    1,
                    None, // compiled_filters
                )
                .unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sqlite_export,
    bench_sqlite_single_row,
    bench_sqlite_real_file
);
criterion_main!(benches);

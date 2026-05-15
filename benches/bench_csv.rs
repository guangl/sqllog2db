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
use std::time::Duration;

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
                    None, // compiled_filters
                )
                .unwrap();
            });
        });
    }

    group.finish();
}

fn bench_csv_real_file(c: &mut Criterion) {
    let real_dir = PathBuf::from("sqllogs");
    if !real_dir.exists() {
        eprintln!("sqllogs/ not found, skipping csv_export_real benchmark");
        return;
    }

    let bench_dir = PathBuf::from("target/bench_csv_real");
    fs::create_dir_all(&bench_dir).unwrap();
    let cfg = make_config(&real_dir, &bench_dir);

    let mut group = c.benchmark_group("csv_export_real");
    // 真实文件慢，减少采样次数；measurement_time 给足单次测量窗口
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
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

/// Micro-benchmark：隔离 CSV 格式化层净开销（不含 `parse_meta`/`parse_performance_metrics`）。
///
/// 输入采用硬编码典型记录（D-03）：包含 ts, ep, sess, trxid, stmt, appname, ip, sql,
/// `EXECTIME`, `ROWCOUNT`, `EXEC_ID`。10000 条相同记录，与 `csv_export/10000` group 对齐，
/// 方便对比格式化层在总开销中的占比。
///
/// 注意：本 group 无 v1.0 baseline。**不要**用 `--baseline v1.0` 对比此 group。
fn bench_csv_format_only(c: &mut Criterion) {
    use dm_database_parser_sqllog::LogParser;
    use dm_database_sqllog2db::exporter::CsvExporter;
    use dm_database_sqllog2db::exporter::Exporter;

    // D-03：硬编码典型记录（中等长度 SQL）
    const LOG_LINE: &str = "2024-01-01 00:00:00.000 (EP[1234] sess:0x0001 user:BENCHUSER trxid:TID001 stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT * FROM t WHERE id = 1. EXECTIME: 10(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n";
    const N: usize = 10_000;

    let bench_dir = PathBuf::from("target/bench_csv_format_only");
    fs::create_dir_all(&bench_dir).unwrap();
    let log_path = bench_dir.join("fmt.log");
    let content: String = LOG_LINE.repeat(N);
    fs::write(&log_path, &content).unwrap();

    // 一次性解析全部 N 条记录到 Vec，benchmark 内只跑格式化
    let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
    let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();
    assert_eq!(
        records.len(),
        N,
        "expected {N} parsed records, got {}",
        records.len()
    );

    // 预解析 meta + pm（这部分开销不计入 benchmark 测量窗口）
    let parsed: Vec<_> = records
        .iter()
        .map(|r| (r, r.parse_meta(), r.parse_performance_metrics()))
        .collect();

    let out_path = bench_dir.join("out.csv");

    let mut group = c.benchmark_group("csv_format_only");
    group.throughput(Throughput::Elements(N as u64));
    group.bench_function(BenchmarkId::from_parameter(N), |b| {
        b.iter(|| {
            let mut exporter = CsvExporter::new(&out_path);
            exporter.initialize().unwrap();
            for (sqllog, meta, pm) in &parsed {
                exporter
                    .export_one_preparsed(sqllog, meta, pm, None)
                    .unwrap();
            }
            exporter.finalize().unwrap();
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_csv_export,
    bench_csv_real_file,
    bench_csv_format_only
);
criterion_main!(benches);

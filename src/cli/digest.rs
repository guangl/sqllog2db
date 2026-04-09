use crate::color;
use crate::config::Config;
use crate::features::fingerprint;
use crate::parser::SqllogParser;
use dm_database_parser_sqllog::LogParser;
use indicatif::{HumanCount, ProgressBar, ProgressStyle};
use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
struct FingerprintAccumulator {
    count: u64,
    total_exec_ms: f64,
    max_exec_ms: f32,
    /// 首次出现时的代表 SQL（未指纹化版本，截取前 120 字符）
    example_sql: String,
    /// 首次出现时的时间戳
    first_seen: String,
}

#[derive(Debug, Serialize)]
pub struct DigestEntry {
    pub rank: usize,
    pub fingerprint: String,
    pub count: u64,
    pub total_exec_ms: f64,
    pub avg_exec_ms: f64,
    pub max_exec_ms: f32,
    pub example_sql: String,
    pub first_seen: String,
}

#[derive(Debug, Serialize)]
struct DigestJson {
    files: usize,
    records: u64,
    errors: u64,
    elapsed_secs: f64,
    rate_per_sec: u64,
    fingerprints: usize,
    entries: Vec<DigestEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Count,
    Exec,
}

impl SortBy {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "count" => Some(Self::Count),
            "exec" => Some(Self::Exec),
            _ => None,
        }
    }
}

pub fn handle_digest(
    cfg: &Config,
    quiet: bool,
    top: Option<usize>,
    sort: SortBy,
    min_count: u64,
    json: bool,
) {
    let start = Instant::now();
    let log_files = match SqllogParser::new(&cfg.sqllog.path).log_files() {
        Ok(files) => files,
        Err(e) => {
            eprintln!("{} {e}", color::red("Error:"));
            return;
        }
    };
    if log_files.is_empty() {
        eprintln!("No log files found in {}", cfg.sqllog.path);
        return;
    }

    let pb = make_progress_bar(quiet);
    let total_files = log_files.len();
    let mut total_records: u64 = 0;
    let mut total_errors: u64 = 0;
    let mut fp_map: HashMap<String, FingerprintAccumulator> = HashMap::new();

    for (idx, log_file) in log_files.iter().enumerate() {
        let file_name = log_file
            .file_name()
            .map_or_else(|| log_file.to_string_lossy(), |n| n.to_string_lossy())
            .into_owned();
        pb.set_prefix(format!("{}/{total_files}", idx + 1));
        pb.set_message(file_name.clone());

        let Ok(parser) = LogParser::from_path(log_file.as_path()) else {
            total_errors += 1;
            continue;
        };

        for result in parser.iter() {
            match result {
                Ok(record) => {
                    let pm = record.parse_performance_metrics();
                    let raw_sql = pm.sql.as_ref();
                    let fp = fingerprint(raw_sql);
                    let ind = record.parse_indicators();
                    let exec_ms = ind.map_or(0.0_f32, |i| i.exectime);

                    let acc = fp_map.entry(fp).or_insert_with(|| FingerprintAccumulator {
                        example_sql: raw_sql.chars().take(120).collect(),
                        first_seen: record.ts.as_ref().to_string(),
                        ..Default::default()
                    });
                    acc.count += 1;
                    acc.total_exec_ms += f64::from(exec_ms);
                    if exec_ms > acc.max_exec_ms {
                        acc.max_exec_ms = exec_ms;
                    }

                    total_records += 1;
                    pb.inc(1);
                }
                Err(_) => total_errors += 1,
            }
        }
    }

    pb.finish_and_clear();
    let elapsed = start.elapsed().as_secs_f64();
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    let rate = if elapsed > 0.0 {
        (total_records as f64 / elapsed) as u64
    } else {
        0
    };

    let mut entries: Vec<DigestEntry> = fp_map
        .into_iter()
        .filter(|(_, acc)| acc.count >= min_count)
        .map(|(fp, acc)| {
            #[allow(clippy::cast_precision_loss)]
            let avg = if acc.count > 0 {
                acc.total_exec_ms / acc.count as f64
            } else {
                0.0
            };
            DigestEntry {
                rank: 0, // 排序后再设置
                fingerprint: fp,
                count: acc.count,
                total_exec_ms: acc.total_exec_ms,
                avg_exec_ms: avg,
                max_exec_ms: acc.max_exec_ms,
                example_sql: acc.example_sql,
                first_seen: acc.first_seen,
            }
        })
        .collect();

    match sort {
        SortBy::Count => entries.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then(a.fingerprint.cmp(&b.fingerprint))
        }),
        SortBy::Exec => entries.sort_by(|a, b| {
            b.total_exec_ms
                .partial_cmp(&a.total_exec_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.fingerprint.cmp(&b.fingerprint))
        }),
    }

    let display_count = top.unwrap_or(entries.len());
    entries.truncate(display_count);
    for (i, entry) in entries.iter_mut().enumerate() {
        entry.rank = i + 1;
    }

    if json {
        print_json(
            total_files,
            total_records,
            total_errors,
            elapsed,
            rate,
            fp_map_len_before_filter(&entries),
            entries,
        );
    } else {
        print_summary(total_files, total_records, total_errors, elapsed, rate);
        print_table(&entries, sort);
    }
}

fn fp_map_len_before_filter(entries: &[DigestEntry]) -> usize {
    // entries 已截断，返回实际展示数即可；调用方用于 JSON 的 fingerprints 字段
    entries.len()
}

fn make_progress_bar(quiet: bool) -> ProgressBar {
    if quiet {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} [{prefix}] {msg} | {human_pos} records [{elapsed_precise}]",
        )
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

fn print_summary(files: usize, records: u64, errors: u64, elapsed: f64, rate: u64) {
    eprintln!(
        "\n{} {} files  {}  {} errors  {:.2}s  {}/s",
        color::cyan("✔"),
        files,
        HumanCount(records),
        if errors > 0 {
            color::yellow(HumanCount(errors))
        } else {
            color::green(HumanCount(0))
        },
        elapsed,
        HumanCount(rate),
    );
}

fn print_table(entries: &[DigestEntry], sort: SortBy) {
    if entries.is_empty() {
        eprintln!("{}", color::dim("No SQL fingerprints found."));
        return;
    }
    let sort_label = match sort {
        SortBy::Count => "count",
        SortBy::Exec => "total exec time",
    };
    eprintln!(
        "\n{} SQL Digest (sorted by {sort_label}):",
        color::cyan("▶")
    );
    eprintln!(
        "  {:<4} {:>8} {:>12} {:>10} {:>10}  {}",
        color::cyan("Rank"),
        color::cyan("Count"),
        color::cyan("Total(ms)"),
        color::cyan("Avg(ms)"),
        color::cyan("Max(ms)"),
        color::cyan("Fingerprint"),
    );
    eprintln!("  {}", color::dim("─".repeat(110)));
    for entry in entries {
        eprintln!(
            "  #{:<3} {:>8} {:>12.1} {:>10.1} {:>10.1}  {}",
            entry.rank,
            HumanCount(entry.count),
            entry.total_exec_ms,
            entry.avg_exec_ms,
            entry.max_exec_ms,
            color::yellow(&entry.fingerprint),
        );
        eprintln!("       {}", color::dim(&entry.example_sql));
    }
}

fn print_json(
    files: usize,
    records: u64,
    errors: u64,
    elapsed: f64,
    rate: u64,
    fingerprints: usize,
    entries: Vec<DigestEntry>,
) {
    let output = DigestJson {
        files,
        records,
        errors,
        elapsed_secs: elapsed,
        rate_per_sec: rate,
        fingerprints,
        entries,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_default()
    );
}

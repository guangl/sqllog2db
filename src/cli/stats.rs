use crate::color;
use crate::config::Config;
use crate::parser::SqllogParser;
use dm_database_parser_sqllog::LogParser;
use indicatif::{HumanCount, ProgressBar, ProgressStyle};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

/// 单文件统计
#[derive(Debug, Serialize)]
struct FileStats {
    name: String,
    records: u64,
    errors: u64,
}

/// `--json` 输出结构
#[derive(Debug, Serialize)]
struct StatsJson {
    files: usize,
    records: u64,
    errors: u64,
    elapsed_secs: f64,
    rate_per_sec: u64,
    per_file: Vec<FileStats>,
    slow_queries: Vec<SlowQueryJson>,
}

#[derive(Debug, Serialize)]
struct SlowQueryJson {
    rank: usize,
    exec_time_ms: f32,
    ts: String,
    sql: String,
    file: String,
}

/// 慢查询条目，通过 `BinaryHeap<Reverse<SlowEntry>>` 维护 Top-N min-heap
#[derive(Debug, Eq, PartialEq)]
struct SlowEntry {
    /// f32 bits — 正数 IEEE 754 bit 顺序与浮点大小顺序一致，可直接用 u32 比较
    exec_time_bits: u32,
    ts: String,
    sql_snippet: String,
    file_name: String,
}

impl SlowEntry {
    fn exec_time_ms(&self) -> f32 {
        f32::from_bits(self.exec_time_bits)
    }
}

impl Ord for SlowEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.exec_time_bits.cmp(&other.exec_time_bits)
    }
}

impl PartialOrd for SlowEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub fn handle_stats(cfg: &Config, quiet: bool, verbose: bool, top: Option<usize>, json: bool) {
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

    let pb = if quiet {
        ProgressBar::hidden()
    } else {
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
    };

    let total_files = log_files.len();
    let mut total_records: u64 = 0;
    let mut total_errors: u64 = 0;
    let mut file_stats: Vec<FileStats> = Vec::with_capacity(total_files);

    // Top-N 慢查询 min-heap：heap 顶始终是当前收集到的最小执行时间
    let top_n = top.unwrap_or(0);
    let mut slow_heap: BinaryHeap<Reverse<SlowEntry>> = BinaryHeap::with_capacity(top_n + 1);

    for (idx, log_file) in log_files.iter().enumerate() {
        let file_name = log_file
            .file_name()
            .map_or_else(|| log_file.to_string_lossy(), |n| n.to_string_lossy())
            .into_owned();

        pb.set_prefix(format!("{}/{total_files}", idx + 1));
        pb.set_message(file_name.clone());

        let Ok(parser) = LogParser::from_path(log_file.as_path()) else {
            total_errors += 1;
            file_stats.push(FileStats {
                name: file_name,
                records: 0,
                errors: 1,
            });
            continue;
        };

        let mut file_records: u64 = 0;
        let mut file_errors: u64 = 0;
        let filters = cfg.features.filters.as_ref().filter(|f| f.has_filters());

        for result in parser.iter() {
            match result {
                Ok(record) => {
                    if let Some(f) = filters {
                        let meta = record.parse_meta();
                        if !f.should_keep(
                            record.ts.as_ref(),
                            &meta.trxid,
                            &meta.client_ip,
                            &meta.sess_id,
                            &meta.thrd_id,
                            &meta.username,
                            &meta.statement,
                            &meta.appname,
                            record.tag.as_deref(),
                        ) {
                            continue;
                        }
                    }

                    if top_n > 0 {
                        if let Some(ind) = record.parse_indicators() {
                            let exec_time = ind.exectime;
                            let should_add = slow_heap.len() < top_n
                                || slow_heap
                                    .peek()
                                    .is_some_and(|Reverse(min)| exec_time > min.exec_time_ms());
                            if should_add {
                                let pm = record.parse_performance_metrics();
                                let sql_snippet: String =
                                    pm.sql.as_ref().chars().take(120).collect();
                                let entry = SlowEntry {
                                    exec_time_bits: exec_time.to_bits(),
                                    ts: record.ts.as_ref().to_string(),
                                    sql_snippet,
                                    file_name: file_name.clone(),
                                };
                                slow_heap.push(Reverse(entry));
                                if slow_heap.len() > top_n {
                                    slow_heap.pop();
                                }
                            }
                        }
                    }

                    file_records += 1;
                    pb.inc(1);
                }
                Err(_) => file_errors += 1,
            }
        }

        total_records += file_records;
        total_errors += file_errors;
        file_stats.push(FileStats {
            name: file_name,
            records: file_records,
            errors: file_errors,
        });
    }

    pb.finish_and_clear();

    if quiet {
        return;
    }

    let elapsed = start.elapsed().as_secs_f64();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let rate = if elapsed > 0.0 {
        (total_records as f64 / elapsed).round() as u64
    } else {
        0
    };

    let mut slow_entries: Vec<SlowEntry> = slow_heap.into_iter().map(|Reverse(e)| e).collect();
    slow_entries.sort_by(|a, b| b.exec_time_bits.cmp(&a.exec_time_bits));

    if json {
        let output = StatsJson {
            files: total_files,
            records: total_records,
            errors: total_errors,
            elapsed_secs: elapsed,
            rate_per_sec: rate,
            per_file: file_stats,
            slow_queries: slow_entries
                .iter()
                .enumerate()
                .map(|(i, e)| SlowQueryJson {
                    rank: i + 1,
                    exec_time_ms: e.exec_time_ms(),
                    ts: e.ts.clone(),
                    sql: e.sql_snippet.clone(),
                    file: e.file_name.clone(),
                })
                .collect(),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&output).unwrap_or_default()
        );
        return;
    }

    // 按文件明细（--verbose）
    if verbose {
        eprintln!();
        eprintln!(
            "  {:<40} {:>10} {:>8}",
            color::cyan("File"),
            color::cyan("Records"),
            color::cyan("Errors"),
        );
        eprintln!("  {}", color::dim("─".repeat(62)));
        for fs in &file_stats {
            eprintln!(
                "  {:<40} {:>10} {:>8}",
                fs.name,
                HumanCount(fs.records),
                if fs.errors > 0 {
                    color::yellow(HumanCount(fs.errors))
                } else {
                    color::green(HumanCount(0))
                },
            );
        }
    }

    // 汇总行
    eprintln!(
        "\n{} Stats — {} files, {} records, {} parse errors  ({}/s, {elapsed:.2}s)",
        color::green("✓"),
        color::green(HumanCount(total_files as u64)),
        color::green(HumanCount(total_records)),
        if total_errors > 0 {
            color::yellow(HumanCount(total_errors))
        } else {
            color::green(HumanCount(0))
        },
        color::green(HumanCount(rate)),
    );

    // Top-N 慢查询
    if top_n > 0 {
        if slow_entries.is_empty() {
            eprintln!("\n{}", color::dim("No execution time data found."));
        } else {
            eprintln!(
                "\n{} Top {} slowest queries:",
                color::cyan("⏱"),
                slow_entries.len()
            );
            eprintln!("  {}", color::dim("─".repeat(100)));
            for (i, entry) in slow_entries.iter().enumerate() {
                eprintln!(
                    "  #{:<3} {:>10.1}ms  {}  [{}]",
                    i + 1,
                    entry.exec_time_ms(),
                    entry.ts,
                    color::dim(&entry.file_name),
                );
                eprintln!("       {}", color::yellow(&entry.sql_snippet));
            }
        }
    }
}

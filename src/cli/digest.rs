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
    count: u32,
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
    skipped_files: usize,
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

pub const DEFAULT_DIGEST_STATE: &str = ".sqllog2db_digest_state.toml";

/// `resume_state_file`: `None` 表示不启用增量模式；`Some(path)` 表示启用并使用该路径作为状态文件。
pub fn handle_digest(
    cfg: &Config,
    quiet: bool,
    top: Option<usize>,
    sort: SortBy,
    min_count: u64,
    json: bool,
    resume_state_file: Option<&str>,
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

    let state_path_opt: Option<std::path::PathBuf> =
        resume_state_file.map(std::path::PathBuf::from);
    let mut resume_state: Option<crate::resume::ResumeState> = state_path_opt
        .as_deref()
        .map(crate::resume::ResumeState::load);

    let pb = make_progress_bar(quiet);
    let total_files = log_files.len();
    let mut total_records: u64 = 0;
    let mut total_errors: u64 = 0;
    let mut skipped_files = 0usize;
    let mut fp_map: HashMap<String, FingerprintAccumulator> = HashMap::new();

    for (idx, log_file) in log_files.iter().enumerate() {
        if let Some(state) = &resume_state {
            if state.is_processed(log_file) {
                skipped_files += 1;
                if !quiet {
                    pb.println(format!(
                        "{} [{}/{}] {} — skipped (already processed)",
                        color::dim("⏭"),
                        idx + 1,
                        total_files,
                        log_file.display(),
                    ));
                }
                continue;
            }
        }

        let file_name = log_file
            .file_name()
            .map_or_else(|| log_file.to_string_lossy(), |n| n.to_string_lossy())
            .into_owned();
        pb.set_prefix(format!("{}/{total_files}", idx + 1));
        pb.set_message(file_name);

        let Ok(parser) = LogParser::from_path(log_file.as_path()) else {
            total_errors += 1;
            continue;
        };

        let records_before = total_records;

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

        let file_records = total_records - records_before;
        if let (Some(state), Some(path)) = (&mut resume_state, &state_path_opt) {
            if let Err(e) = state.mark_processed(log_file, file_records) {
                eprintln!(
                    "{} Failed to mark file as processed: {e}",
                    color::yellow("Warning:")
                );
            } else if let Err(e) = state.save(path) {
                eprintln!(
                    "{} Failed to save resume state: {e}",
                    color::yellow("Warning:")
                );
            }
        }
    }

    pb.finish_and_clear();
    let elapsed = start.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let rate = total_records / elapsed.as_secs().max(1);

    let mut entries: Vec<DigestEntry> = fp_map
        .into_iter()
        .filter(|(_, acc)| u64::from(acc.count) >= min_count)
        .map(|(fp, acc)| {
            let avg = if acc.count > 0 {
                acc.total_exec_ms / f64::from(acc.count)
            } else {
                0.0
            };
            DigestEntry {
                rank: 0, // 排序后再设置
                fingerprint: fp,
                count: u64::from(acc.count),
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
            total_files - skipped_files,
            total_records,
            total_errors,
            elapsed_secs,
            rate,
            fp_map_len_before_filter(&entries),
            skipped_files,
            entries,
        );
    } else {
        print_summary(
            total_files - skipped_files,
            total_records,
            total_errors,
            elapsed_secs,
            rate,
            skipped_files,
        );
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

fn print_summary(files: usize, records: u64, errors: u64, elapsed: f64, rate: u64, skipped: usize) {
    let skip_label = if skipped > 0 {
        format!("  ({} skipped)", color::dim(HumanCount(skipped as u64)))
    } else {
        String::new()
    };
    eprintln!(
        "\n{} {} files{}  {}  {} errors  {:.2}s  {}/s",
        color::cyan("✔"),
        files,
        skip_label,
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
    skipped_files: usize,
    entries: Vec<DigestEntry>,
) {
    let output = DigestJson {
        files,
        records,
        errors,
        elapsed_secs: elapsed,
        rate_per_sec: rate,
        fingerprints,
        skipped_files,
        entries,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_by_parse_count() {
        assert_eq!(SortBy::parse("count"), Some(SortBy::Count));
    }

    #[test]
    fn test_sort_by_parse_exec() {
        assert_eq!(SortBy::parse("exec"), Some(SortBy::Exec));
    }

    #[test]
    fn test_sort_by_parse_invalid() {
        assert_eq!(SortBy::parse("unknown"), None);
        assert_eq!(SortBy::parse(""), None);
    }

    #[test]
    fn test_fp_map_len_before_filter() {
        let entries = vec![
            DigestEntry {
                rank: 1,
                fingerprint: "fp1".into(),
                count: 5,
                total_exec_ms: 10.0,
                avg_exec_ms: 2.0,
                max_exec_ms: 5.0,
                example_sql: "SELECT ?".into(),
                first_seen: "2025-01-01".into(),
            },
            DigestEntry {
                rank: 2,
                fingerprint: "fp2".into(),
                count: 3,
                total_exec_ms: 6.0,
                avg_exec_ms: 2.0,
                max_exec_ms: 3.0,
                example_sql: "INSERT ?".into(),
                first_seen: "2025-01-02".into(),
            },
        ];
        assert_eq!(fp_map_len_before_filter(&entries), 2);
    }

    #[test]
    fn test_fingerprint_accumulator_default() {
        let acc = FingerprintAccumulator::default();
        assert_eq!(acc.count, 0);
        assert!(acc.total_exec_ms.abs() < f64::EPSILON);
        assert!(acc.max_exec_ms.abs() < f32::EPSILON);
        assert!(acc.example_sql.is_empty());
        assert!(acc.first_seen.is_empty());
    }

    #[test]
    fn test_print_table_empty() {
        // empty entries → prints "No SQL fingerprints found."
        print_table(&[], SortBy::Count);
    }

    #[test]
    fn test_print_table_exec_sort() {
        let entry = DigestEntry {
            rank: 1,
            fingerprint: "SELECT ?".into(),
            count: 2,
            total_exec_ms: 4.0,
            avg_exec_ms: 2.0,
            max_exec_ms: 3.0,
            example_sql: "SELECT 1".into(),
            first_seen: "2025-01-01".into(),
        };
        // SortBy::Exec branch → sort_label = "total exec time"
        print_table(&[entry], SortBy::Exec);
    }
}

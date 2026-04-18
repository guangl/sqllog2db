use crate::color;
use crate::config::Config;
use crate::features::filters::RecordMeta;
use crate::parser::SqllogParser;
use dm_database_parser_sqllog::{LogParser, MetaParts};
use indicatif::{HumanCount, ProgressBar, ProgressStyle};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
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
    skipped_files: usize,
    per_file: Vec<FileStats>,
    slow_queries: Vec<SlowQueryJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    group_sections: Vec<GroupSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_buckets: Option<TimeBucketSection>,
}

#[derive(Debug, Serialize)]
struct SlowQueryJson {
    rank: usize,
    exec_time_ms: f32,
    ts: String,
    sql: String,
    file: String,
}

// ── 聚合维度 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupBy {
    User,
    App,
    Ip,
}

impl GroupBy {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "app" => Some(Self::App),
            "ip" => Some(Self::Ip),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::User => "User",
            Self::App => "App",
            Self::Ip => "IP",
        }
    }

    fn key<'a>(self, meta: &'a MetaParts) -> &'a str {
        match self {
            Self::User => meta.username.as_ref(),
            Self::App => meta.appname.as_ref(),
            Self::Ip => meta.client_ip.as_ref(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct GroupAccumulator {
    count: u32,
    total_exec_ms: f64,
    max_exec_ms: f32,
}

#[derive(Debug, Serialize)]
struct GroupEntry {
    key: String,
    count: u64,
    total_exec_ms: f64,
    avg_exec_ms: f64,
    max_exec_ms: f32,
}

impl GroupEntry {
    fn from_acc(key: String, acc: GroupAccumulator) -> Self {
        let avg = if acc.count > 0 {
            acc.total_exec_ms / f64::from(acc.count)
        } else {
            0.0
        };
        Self {
            key,
            count: u64::from(acc.count),
            total_exec_ms: acc.total_exec_ms,
            avg_exec_ms: avg,
            max_exec_ms: acc.max_exec_ms,
        }
    }
}

#[derive(Debug, Serialize)]
struct GroupSection {
    field: String,
    entries: Vec<GroupEntry>,
}

// ── 时间分桶 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bucket {
    Hour,
    Minute,
}

impl Bucket {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "hour" => Some(Self::Hour),
            "minute" => Some(Self::Minute),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Hour => "hour",
            Self::Minute => "minute",
        }
    }

    /// 从时间戳字符串提取分桶 key（字符串截断，零分配）
    fn truncate(self, ts: &str) -> &str {
        let len = match self {
            Self::Hour => 13,   // "2025-01-15 10"
            Self::Minute => 16, // "2025-01-15 10:30"
        };
        &ts[..ts.len().min(len)]
    }
}

#[derive(Debug, Default)]
struct BucketAccumulator {
    count: u32,
    total_exec_ms: f64,
    max_exec_ms: f32,
}

#[derive(Debug, Serialize)]
struct BucketEntry {
    time: String,
    count: u64,
    total_exec_ms: f64,
    avg_exec_ms: f64,
    max_exec_ms: f32,
}

#[derive(Debug, Serialize)]
struct TimeBucketSection {
    granularity: String,
    entries: Vec<BucketEntry>,
}

// ── 慢查询 ───────────────────────────────────────────────────────────────────

#[derive(Debug, Eq, PartialEq)]
struct SlowEntry {
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

// ── 主入口 ───────────────────────────────────────────────────────────────────

pub const DEFAULT_STATS_STATE: &str = ".sqllog2db_stats_state.toml";

/// `resume_state_file`: `None` 表示不启用增量模式；`Some(path)` 表示启用并使用该路径作为状态文件。
pub fn handle_stats(
    cfg: &Config,
    quiet: bool,
    verbose: bool,
    top: Option<usize>,
    json: bool,
    group_by: &[String],
    bucket: Option<&str>,
    resume_state_file: Option<&str>,
) {
    let Some(group_fields) = parse_group_fields(group_by) else {
        return;
    };
    let bucket_field: Option<Bucket> = match bucket {
        None => None,
        Some(s) => {
            if let Some(b) = Bucket::from_str(s) {
                Some(b)
            } else {
                eprintln!(
                    "{} Unknown bucket granularity '{}'. Valid values: hour, minute",
                    color::red("Error:"),
                    s
                );
                return;
            }
        }
    };

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
    let mut file_stats: Vec<FileStats> = Vec::with_capacity(total_files);
    let top_n = top.unwrap_or(0);
    let mut slow_heap: BinaryHeap<Reverse<SlowEntry>> = BinaryHeap::with_capacity(top_n + 1);
    let mut group_maps: Vec<HashMap<String, GroupAccumulator>> =
        group_fields.iter().map(|_| HashMap::new()).collect();
    let mut bucket_map: BTreeMap<String, BucketAccumulator> = BTreeMap::new();

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

        let (file_records, file_errors) = process_file(
            &parser,
            &mut ProcessFileCtx {
                cfg,
                file_name: &file_name,
                top_n,
                slow_heap: &mut slow_heap,
                group_fields: &group_fields,
                group_maps: &mut group_maps,
                bucket_field,
                bucket_map: &mut bucket_map,
                pb: &pb,
            },
        );

        total_records += file_records;
        total_errors += file_errors;
        file_stats.push(FileStats {
            name: file_name,
            records: file_records,
            errors: file_errors,
        });

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
    if quiet {
        return;
    }

    let elapsed = start.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let rate = total_records / elapsed.as_secs().max(1);

    let mut slow_entries: Vec<SlowEntry> = slow_heap.into_iter().map(|Reverse(e)| e).collect();
    slow_entries.sort_by_key(|e| std::cmp::Reverse(e.exec_time_bits));

    let group_sections = build_group_sections(&group_fields, group_maps);
    let time_bucket_section = build_bucket_section(bucket_field, bucket_map);

    if json {
        let slow_queries = slow_entries
            .iter()
            .enumerate()
            .map(|(i, e)| SlowQueryJson {
                rank: i + 1,
                exec_time_ms: e.exec_time_ms(),
                ts: e.ts.clone(),
                sql: e.sql_snippet.clone(),
                file: e.file_name.clone(),
            })
            .collect();
        print_json(&StatsJson {
            files: total_files - skipped_files,
            records: total_records,
            errors: total_errors,
            elapsed_secs,
            rate_per_sec: rate,
            skipped_files,
            per_file: file_stats,
            slow_queries,
            group_sections,
            time_buckets: time_bucket_section,
        });
        return;
    }

    if verbose {
        print_file_table(&file_stats);
    }

    let skip_label = if skipped_files > 0 {
        format!(", {} skipped", color::dim(HumanCount(skipped_files as u64)))
    } else {
        String::new()
    };
    eprintln!(
        "\n{} Stats — {} files{}, {} records, {} parse errors  ({}/s, {elapsed_secs:.2}s)",
        color::green("✓"),
        color::green(HumanCount((total_files - skipped_files) as u64)),
        skip_label,
        color::green(HumanCount(total_records)),
        if total_errors > 0 {
            color::yellow(HumanCount(total_errors))
        } else {
            color::green(HumanCount(0))
        },
        color::green(HumanCount(rate)),
    );

    for section in &group_sections {
        let field = GroupBy::from_str(&section.field).unwrap_or(GroupBy::User);
        print_group_table(&section.entries, field);
    }
    if let Some(ref section) = time_bucket_section {
        print_bucket_table(section);
    }
    print_slow_queries(&slow_entries, top_n);
}

// ── 解析辅助 ─────────────────────────────────────────────────────────────────

fn parse_group_fields(raw: &[String]) -> Option<Vec<GroupBy>> {
    let mut fields: Vec<GroupBy> = Vec::new();
    for s in raw {
        match GroupBy::from_str(s) {
            Some(g) if !fields.contains(&g) => fields.push(g),
            Some(_) => {}
            None => {
                eprintln!(
                    "{} Unknown group-by field '{}'. Valid values: user, app, ip",
                    color::red("Error:"),
                    s
                );
                return None;
            }
        }
    }
    Some(fields)
}

// ── 处理单个文件 ─────────────────────────────────────────────────────────────

struct ProcessFileCtx<'a> {
    cfg: &'a Config,
    file_name: &'a str,
    top_n: usize,
    slow_heap: &'a mut BinaryHeap<Reverse<SlowEntry>>,
    group_fields: &'a [GroupBy],
    group_maps: &'a mut [HashMap<String, GroupAccumulator>],
    bucket_field: Option<Bucket>,
    bucket_map: &'a mut BTreeMap<String, BucketAccumulator>,
    pb: &'a ProgressBar,
}

// stats 子命令使用 FiltersFeature::should_keep（OR 语义）做统计过滤，
// 与热路径导出的 AND 语义无关，此处 OR 语义是预期行为。
#[allow(deprecated)]
fn process_file(
    parser: &dm_database_parser_sqllog::LogParser,
    ctx: &mut ProcessFileCtx,
) -> (u64, u64) {
    let mut file_records: u64 = 0;
    let mut file_errors: u64 = 0;
    let filters = ctx
        .cfg
        .features
        .filters
        .as_ref()
        .filter(|f| f.has_filters());
    let need_meta = filters.is_some() || !ctx.group_fields.is_empty();
    let need_ind = ctx.top_n > 0 || !ctx.group_fields.is_empty() || ctx.bucket_field.is_some();

    for result in parser.iter() {
        match result {
            Ok(record) => {
                let meta: Option<MetaParts<'_>> = if need_meta {
                    Some(record.parse_meta())
                } else {
                    None
                };

                if let (Some(f), Some(m)) = (filters, &meta) {
                    if !f.should_keep(
                        record.ts.as_ref(),
                        &RecordMeta {
                            trxid: m.trxid.as_ref(),
                            ip: m.client_ip.as_ref(),
                            sess: m.sess_id.as_ref(),
                            thrd: m.thrd_id.as_ref(),
                            user: m.username.as_ref(),
                            stmt: m.statement.as_ref(),
                            app: m.appname.as_ref(),
                            tag: record.tag.as_deref(),
                        },
                    ) {
                        continue;
                    }
                }

                let ind = if need_ind {
                    record.parse_indicators()
                } else {
                    None
                };

                for (gb, map) in ctx.group_fields.iter().zip(ctx.group_maps.iter_mut()) {
                    let raw_key = meta.as_ref().map_or("", |m| gb.key(m));
                    let key = if raw_key.is_empty() {
                        "(unknown)".to_owned()
                    } else {
                        raw_key.to_owned()
                    };
                    let acc = map.entry(key).or_default();
                    acc.count += 1;
                    if let Some(ref i) = ind {
                        acc.total_exec_ms += f64::from(i.exectime);
                        if i.exectime > acc.max_exec_ms {
                            acc.max_exec_ms = i.exectime;
                        }
                    }
                }

                if let Some(bkt) = ctx.bucket_field {
                    let key = bkt.truncate(record.ts.as_ref()).to_owned();
                    let acc = ctx.bucket_map.entry(key).or_default();
                    acc.count += 1;
                    if let Some(ref i) = ind {
                        acc.total_exec_ms += f64::from(i.exectime);
                        if i.exectime > acc.max_exec_ms {
                            acc.max_exec_ms = i.exectime;
                        }
                    }
                }

                if ctx.top_n > 0 {
                    if let Some(ref i) = ind {
                        push_slow_entry(
                            ctx.slow_heap,
                            ctx.top_n,
                            i.exectime,
                            &record,
                            ctx.file_name,
                        );
                    }
                }

                file_records += 1;
                ctx.pb.inc(1);
            }
            Err(_) => file_errors += 1,
        }
    }
    (file_records, file_errors)
}

fn push_slow_entry(
    slow_heap: &mut BinaryHeap<Reverse<SlowEntry>>,
    top_n: usize,
    exec_time: f32,
    record: &dm_database_parser_sqllog::Sqllog,
    file_name: &str,
) {
    let should_add = slow_heap.len() < top_n
        || slow_heap
            .peek()
            .is_some_and(|Reverse(min)| exec_time > min.exec_time_ms());
    if should_add {
        let pm = record.parse_performance_metrics();
        let sql_snippet: String = pm.sql.as_ref().chars().take(120).collect();
        slow_heap.push(Reverse(SlowEntry {
            exec_time_bits: exec_time.to_bits(),
            ts: record.ts.as_ref().to_string(),
            sql_snippet,
            file_name: file_name.to_owned(),
        }));
        if slow_heap.len() > top_n {
            slow_heap.pop();
        }
    }
}

// ── 构建结果 ─────────────────────────────────────────────────────────────────

fn build_group_sections(
    group_fields: &[GroupBy],
    group_maps: Vec<HashMap<String, GroupAccumulator>>,
) -> Vec<GroupSection> {
    group_fields
        .iter()
        .zip(group_maps)
        .map(|(gb, map)| {
            let mut entries: Vec<GroupEntry> = map
                .into_iter()
                .map(|(key, acc)| GroupEntry::from_acc(key, acc))
                .collect();
            entries.sort_by(|a, b| b.count.cmp(&a.count).then(a.key.cmp(&b.key)));
            GroupSection {
                field: gb.label().to_lowercase(),
                entries,
            }
        })
        .collect()
}

fn build_bucket_section(
    bucket_field: Option<Bucket>,
    bucket_map: BTreeMap<String, BucketAccumulator>,
) -> Option<TimeBucketSection> {
    let bkt = bucket_field?;
    if bucket_map.is_empty() {
        return None;
    }
    let entries = bucket_map
        .into_iter()
        .map(|(time, acc)| {
            let avg = if acc.count > 0 {
                acc.total_exec_ms / f64::from(acc.count)
            } else {
                0.0
            };
            BucketEntry {
                time,
                count: u64::from(acc.count),
                total_exec_ms: acc.total_exec_ms,
                avg_exec_ms: avg,
                max_exec_ms: acc.max_exec_ms,
            }
        })
        .collect();
    Some(TimeBucketSection {
        granularity: bkt.label().to_string(),
        entries,
    })
}

// ── 打印 ─────────────────────────────────────────────────────────────────────

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

fn print_file_table(file_stats: &[FileStats]) {
    eprintln!();
    eprintln!(
        "  {:<40} {:>10} {:>8}",
        color::cyan("File"),
        color::cyan("Records"),
        color::cyan("Errors")
    );
    eprintln!("  {}", color::dim("─".repeat(62)));
    for fs in file_stats {
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

fn print_group_table(entries: &[GroupEntry], field: GroupBy) {
    eprintln!("\n{} Group by {}:", color::cyan("▶"), field.label());
    eprintln!(
        "  {:<30} {:>10} {:>14} {:>10} {:>10}",
        color::cyan(field.label()),
        color::cyan("Count"),
        color::cyan("Total(ms)"),
        color::cyan("Avg(ms)"),
        color::cyan("Max(ms)"),
    );
    eprintln!("  {}", color::dim("─".repeat(78)));
    for entry in entries {
        eprintln!(
            "  {:<30} {:>10} {:>14.1} {:>10.1} {:>10.1}",
            if entry.key.is_empty() {
                "(unknown)"
            } else {
                &entry.key
            },
            HumanCount(entry.count),
            entry.total_exec_ms,
            entry.avg_exec_ms,
            entry.max_exec_ms,
        );
    }
}

const BAR_WIDTH: usize = 20;

fn print_bucket_table(section: &TimeBucketSection) {
    let max_count = section.entries.iter().map(|e| e.count).max().unwrap_or(1);
    eprintln!("\n{} Records by {}:", color::cyan("▶"), section.granularity);
    eprintln!(
        "  {:<19} {:>10} {:>10} {:>10}",
        color::cyan("Time"),
        color::cyan("Count"),
        color::cyan("Avg(ms)"),
        color::cyan("Max(ms)"),
    );
    eprintln!("  {}", color::dim("─".repeat(65)));
    for entry in &section.entries {
        let bar = make_bar(entry.count, max_count);
        eprintln!(
            "  {:<19} {:>10} {:>10.1} {:>10.1}  {}",
            entry.time,
            HumanCount(entry.count),
            entry.avg_exec_ms,
            entry.max_exec_ms,
            color::cyan(bar),
        );
    }
}

fn make_bar(count: u64, max_count: u64) -> String {
    if max_count == 0 {
        return String::new();
    }
    let filled =
        usize::try_from(count.min(max_count) * BAR_WIDTH as u64 / max_count).unwrap_or(BAR_WIDTH);
    "█".repeat(filled)
}

fn print_slow_queries(slow_entries: &[SlowEntry], top_n: usize) {
    if top_n == 0 {
        return;
    }
    if slow_entries.is_empty() {
        eprintln!("\n{}", color::dim("No execution time data found."));
        return;
    }
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

fn print_json(output: &StatsJson) {
    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ── GroupBy ───────────────────────────────────────────────────
    #[test]
    fn test_group_by_from_str_valid() {
        assert_eq!(GroupBy::from_str("user"), Some(GroupBy::User));
        assert_eq!(GroupBy::from_str("app"), Some(GroupBy::App));
        assert_eq!(GroupBy::from_str("ip"), Some(GroupBy::Ip));
    }

    #[test]
    fn test_group_by_from_str_invalid() {
        assert_eq!(GroupBy::from_str("unknown"), None);
        assert_eq!(GroupBy::from_str(""), None);
    }

    #[test]
    fn test_group_by_label() {
        assert_eq!(GroupBy::User.label(), "User");
        assert_eq!(GroupBy::App.label(), "App");
        assert_eq!(GroupBy::Ip.label(), "IP");
    }

    // ── Bucket ────────────────────────────────────────────────────
    #[test]
    fn test_bucket_from_str_valid() {
        assert_eq!(Bucket::from_str("hour"), Some(Bucket::Hour));
        assert_eq!(Bucket::from_str("minute"), Some(Bucket::Minute));
    }

    #[test]
    fn test_bucket_from_str_invalid() {
        assert_eq!(Bucket::from_str("day"), None);
        assert_eq!(Bucket::from_str(""), None);
    }

    #[test]
    fn test_bucket_label() {
        assert_eq!(Bucket::Hour.label(), "hour");
        assert_eq!(Bucket::Minute.label(), "minute");
    }

    #[test]
    fn test_bucket_truncate_hour() {
        let ts = "2025-01-15 10:30:28";
        assert_eq!(Bucket::Hour.truncate(ts), "2025-01-15 10");
    }

    #[test]
    fn test_bucket_truncate_minute() {
        let ts = "2025-01-15 10:30:28";
        assert_eq!(Bucket::Minute.truncate(ts), "2025-01-15 10:30");
    }

    #[test]
    fn test_bucket_truncate_short_ts() {
        // Shorter than expected → clamped to full string
        let ts = "2025";
        assert_eq!(Bucket::Hour.truncate(ts), "2025");
    }

    // ── GroupEntry ────────────────────────────────────────────────
    #[test]
    fn test_group_entry_from_acc_zero_count() {
        let acc = GroupAccumulator {
            count: 0,
            total_exec_ms: 0.0,
            max_exec_ms: 0.0,
        };
        let entry = GroupEntry::from_acc("key".into(), acc);
        assert!(entry.avg_exec_ms.abs() < f64::EPSILON);
        assert_eq!(entry.count, 0);
    }

    #[test]
    fn test_group_entry_from_acc_nonzero() {
        let acc = GroupAccumulator {
            count: 4,
            total_exec_ms: 8.0,
            max_exec_ms: 5.0,
        };
        let entry = GroupEntry::from_acc("u1".into(), acc);
        assert!((entry.avg_exec_ms - 2.0).abs() < f64::EPSILON);
        assert_eq!(entry.count, 4);
    }

    // ── make_bar ─────────────────────────────────────────────────
    #[test]
    fn test_make_bar_zero_max() {
        assert_eq!(make_bar(0, 0), "");
    }

    #[test]
    fn test_make_bar_full() {
        let bar = make_bar(10, 10);
        assert_eq!(bar.chars().count(), BAR_WIDTH);
    }

    #[test]
    fn test_make_bar_half() {
        let bar = make_bar(5, 10);
        assert_eq!(bar.chars().count(), BAR_WIDTH / 2);
    }

    // ── parse_group_fields ────────────────────────────────────────
    #[test]
    fn test_parse_group_fields_empty() {
        let result = parse_group_fields(&[]);
        assert_eq!(result, Some(vec![]));
    }

    #[test]
    fn test_parse_group_fields_valid() {
        let result = parse_group_fields(&["user".to_string(), "ip".to_string()]);
        assert_eq!(result, Some(vec![GroupBy::User, GroupBy::Ip]));
    }

    #[test]
    fn test_parse_group_fields_duplicate_deduped() {
        let result = parse_group_fields(&["app".to_string(), "app".to_string()]);
        assert_eq!(result, Some(vec![GroupBy::App]));
    }

    #[test]
    fn test_parse_group_fields_unknown_returns_none() {
        let result = parse_group_fields(&["bad".to_string()]);
        assert!(result.is_none());
    }

    // ── SlowEntry ─────────────────────────────────────────────────
    #[test]
    fn test_slow_entry_exec_time_ms_roundtrip() {
        let val = 123.45_f32;
        let entry = SlowEntry {
            exec_time_bits: val.to_bits(),
            ts: "t".into(),
            sql_snippet: "s".into(),
            file_name: "f".into(),
        };
        assert!((entry.exec_time_ms() - val).abs() < 1e-4);
    }

    #[test]
    fn test_slow_entry_ordering() {
        let small = SlowEntry {
            exec_time_bits: 1.0_f32.to_bits(),
            ts: "t".into(),
            sql_snippet: "s".into(),
            file_name: "f".into(),
        };
        let large = SlowEntry {
            exec_time_bits: 100.0_f32.to_bits(),
            ts: "t".into(),
            sql_snippet: "s".into(),
            file_name: "f".into(),
        };
        assert!(small < large);
        assert_eq!(small.partial_cmp(&large), Some(std::cmp::Ordering::Less));
    }

    // ── build_bucket_section ──────────────────────────────────────
    #[test]
    fn test_build_bucket_section_none_field() {
        let result = build_bucket_section(None, BTreeMap::new());
        assert!(result.is_none());
    }

    #[test]
    fn test_build_bucket_section_empty_map() {
        let result = build_bucket_section(Some(Bucket::Hour), BTreeMap::new());
        assert!(result.is_none());
    }

    #[test]
    fn test_build_bucket_section_with_data() {
        let mut map = BTreeMap::new();
        map.insert(
            "2025-01-15 10".to_string(),
            BucketAccumulator {
                count: 5,
                total_exec_ms: 50.0,
                max_exec_ms: 20.0,
            },
        );
        let result = build_bucket_section(Some(Bucket::Hour), map);
        assert!(result.is_some());
        let section = result.unwrap();
        assert_eq!(section.granularity, "hour");
        assert_eq!(section.entries.len(), 1);
        assert_eq!(section.entries[0].count, 5);
        assert!((section.entries[0].avg_exec_ms - 10.0).abs() < f64::EPSILON);
    }

    // ── build_group_sections ──────────────────────────────────────
    #[test]
    fn test_build_group_sections_empty() {
        let result = build_group_sections(&[], vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_build_group_sections_with_data() {
        let mut map: HashMap<String, GroupAccumulator> = HashMap::new();
        map.insert(
            "alice".to_string(),
            GroupAccumulator {
                count: 3,
                total_exec_ms: 6.0,
                max_exec_ms: 4.0,
            },
        );
        let result = build_group_sections(&[GroupBy::User], vec![map]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].field, "user");
        assert_eq!(result[0].entries[0].key, "alice");
    }

    // ── print_slow_queries ────────────────────────────────────────
    #[test]
    fn test_print_slow_queries_zero_top_n() {
        // top_n == 0 → returns immediately, no panic
        print_slow_queries(&[], 0);
    }

    #[test]
    fn test_print_slow_queries_empty_entries_with_top() {
        // top_n > 0 but no entries → prints "No execution time data"
        print_slow_queries(&[], 3);
    }
}

use crate::color;
use crate::config::Config;
use crate::error::ParserError;
use crate::error::{Error, Result};
use crate::exporter::{CsvExporter, ExporterManager};
use crate::features::filters::RecordMeta;
use crate::features::replace_parameters::ParamBuffer;
use crate::features::{LogProcessor, Pipeline};
use crate::parser::SqllogParser;
use ahash::HashSet as AHashSet;
use compact_str::CompactString;
use dm_database_parser_sqllog::{LogParser, MetaParts};
use indicatif::{HumanCount, ProgressBar, ProgressStyle};
use log::{info, warn};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// 构建处理器管线
fn build_pipeline(cfg: &Config) -> Pipeline {
    let mut pipeline = Pipeline::new();

    if let Some(f) = &cfg.features.filters {
        if f.has_filters() {
            pipeline.add(Box::new(FilterProcessor::new(f.clone())));
        }
    }

    pipeline
}

#[derive(Debug)]
struct FilterProcessor {
    filter: crate::features::FiltersFeature,
    /// 预计算：`filter.meta.has_filters()` 的结果。避免每条记录重复扫描 8 个 Option 字段，
    /// 并在无元数据过滤时跳过 `RecordMeta` 结构体的构造（8 次字段载入）。
    has_meta_filters: bool,
}

impl FilterProcessor {
    fn new(filter: crate::features::FiltersFeature) -> Self {
        let has_meta_filters = filter.meta.has_filters();
        Self {
            filter,
            has_meta_filters,
        }
    }
}

impl LogProcessor for FilterProcessor {
    fn process(&self, record: &dm_database_parser_sqllog::Sqllog) -> bool {
        let meta = record.parse_meta();
        self.process_with_meta(record, &meta)
    }

    /// 热路径重载：复用调用方已解析的 `MetaParts`，消除 `parse_meta()` 重复调用。
    ///
    /// 时间过滤在前（无需构造 `RecordMeta`），之后用预计算的 `has_meta_filters`
    /// 快速判断是否需要进入元数据过滤 —— 过滤器只含时间范围时直接返回 true。
    fn process_with_meta(
        &self,
        record: &dm_database_parser_sqllog::Sqllog,
        meta: &MetaParts<'_>,
    ) -> bool {
        let ts = record.ts.as_ref();

        // 时间过滤：无需构造 RecordMeta
        if let Some(start) = &self.filter.meta.start_ts {
            if ts < start.as_str() && !ts.starts_with(start.as_str()) {
                return false;
            }
        }
        if let Some(end) = &self.filter.meta.end_ts {
            if ts > end.as_str() && !ts.starts_with(end.as_str()) {
                return false;
            }
        }

        // 快速路径：无元数据过滤 → 直接通过，跳过 RecordMeta 构造
        if !self.has_meta_filters {
            return true;
        }

        self.filter.meta.should_keep(&RecordMeta {
            trxid: meta.trxid.as_ref(),
            ip: meta.client_ip.as_ref(),
            sess: meta.sess_id.as_ref(),
            thrd: meta.thrd_id.as_ref(),
            user: meta.username.as_ref(),
            stmt: meta.statement.as_ref(),
            app: meta.appname.as_ref(),
            tag: record.tag.as_deref(),
        })
    }
}

/// 处理单个日志文件，返回本文件实际导出的记录数。
///
/// `limit`: 最多再导出多少条记录（跨文件的剩余配额），`None` 表示不限制。
/// `reset_pb`: 是否在文件开始时重置进度条计数；并行模式传 `false`，避免多线程互相重置。
fn process_log_file(
    file_path: &str,
    file_index: usize,
    total_files: usize,
    exporter_manager: &mut ExporterManager,
    pipeline: &Pipeline,
    pb: &ProgressBar,
    limit: Option<usize>,
    interrupted: &Arc<AtomicBool>,
    do_normalize: bool,
    placeholder_override: Option<bool>,
    params_buffer: &mut ParamBuffer,
    ns_scratch: &mut Vec<u8>,
    reset_pb: bool,
) -> Result<usize> {
    // 清除上一个文件留下的残余参数，同时复用已分配的 HashMap 容量。
    params_buffer.clear();

    let file_start = Instant::now();

    let file_name = std::path::Path::new(file_path).file_name().map_or_else(
        || file_path.to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    if reset_pb {
        pb.set_prefix(format!("{file_index}/{total_files}"));
        pb.set_message(file_name.clone());
        pb.reset();
    }

    let parser = LogParser::from_path(file_path).map_err(|e| {
        Error::Parser(ParserError::InvalidPath {
            path: file_path.into(),
            reason: format!("{e}"),
        })
    })?;

    let mut records_in_file = 0usize;
    let mut errors_in_file = 0usize;
    // 用于攒批更新进度条，避免每条记录都触发原子操作
    let mut pb_pending: u64 = 0;

    'outer: for result in parser.iter() {
        match result {
            Ok(record) => {
                // 管线为空：零开销快速路径，所有记录都通过，不提前解析 meta。
                // 管线非空：提前解析 meta，与管线过滤器共享，消除 FilterProcessor
                //           内部的重复 parse_meta() 调用（对 pipeline_passthrough
                //           场景可减少约 50% 的 parse_meta 调用次数）。
                let (passes, cached_meta) = if pipeline.is_empty() {
                    (true, None)
                } else {
                    let meta = record.parse_meta();
                    let ok = pipeline.run_with_meta(&record, &meta);
                    (ok, Some(meta))
                };

                // PARAMS 记录（无 tag）在 do_normalize 时无论是否通过过滤都必须
                // 更新 params_buffer，以便后续匹配 DML 记录能正确替换参数。
                let needs_pm = passes || (do_normalize && record.tag.is_none());
                if needs_pm {
                    // 无管线时首次解析 meta；有管线时复用已解析结果，零额外开销。
                    let meta = cached_meta.unwrap_or_else(|| record.parse_meta());

                    if passes {
                        // DML 或通过过滤的 PARAMS：需要完整 pm 用于导出。
                        let pm = record.parse_performance_metrics();

                        // 快速路径：params_buffer 为空且当前是 DML 记录（有 tag），
                        // 则不可能存在待替换参数，完全跳过 compute_normalized。
                        let ns = if do_normalize
                            && (!params_buffer.is_empty() || record.tag.is_none())
                        {
                            crate::features::compute_normalized(
                                &record,
                                &meta,
                                pm.sql.as_ref(),
                                params_buffer,
                                placeholder_override,
                                ns_scratch,
                            )
                        } else {
                            None
                        };

                        // 检查是否即将超出本文件的剩余配额
                        if let Some(remaining) = limit {
                            if records_in_file >= remaining {
                                break 'outer;
                            }
                        }

                        exporter_manager.export_one_preparsed(&record, &meta, &pm, ns)?;
                        records_in_file += 1;
                        pb_pending += 1;

                        // 每 4096 条更新一次进度条（减少原子操作频率）
                        if pb_pending >= 4096 {
                            pb.inc(pb_pending);
                            pb_pending = 0;
                        }

                        // 每 1024 条检查一次中断信号
                        if records_in_file.trailing_zeros() >= 10
                            && interrupted.load(Ordering::Relaxed)
                        {
                            break 'outer;
                        }
                    } else {
                        // 被过滤掉的 PARAMS 记录（needs_pm 成立说明 do_normalize &&
                        // record.tag.is_none() 为真）：对 PARAMS 记录而言
                        // pm.sql ≡ record.body()，直接复用，省去 parse_performance_metrics()。
                        crate::features::compute_normalized(
                            &record,
                            &meta,
                            record.body().as_ref(),
                            params_buffer,
                            placeholder_override,
                            ns_scratch,
                        );
                    }
                }
            }
            Err(e) => {
                errors_in_file += 1;
                log::warn!("{file_path} | {e:?}");
            }
        }
    }

    // 将剩余未上报的进度刷新到进度条
    if pb_pending > 0 {
        pb.inc(pb_pending);
    }

    let elapsed = file_start.elapsed().as_secs_f64();
    info!(
        "File {file_path}: {records_in_file} records, {errors_in_file} errors, total {elapsed:.2}s",
    );

    let errors_label = if errors_in_file > 0 {
        color::yellow(format!(", {errors_in_file} errors"))
    } else {
        String::new()
    };
    pb.println(format!(
        "{} [{file_index}/{total_files}] {file_path} — {}{errors_label}, {elapsed:.2}s",
        color::green("✓"),
        color::green(HumanCount(records_in_file as u64)),
    ));

    Ok(records_in_file)
}

/// 扫描单个日志文件，返回满足事务级过滤条件的去重 `trxid` 列表。
///
/// 文件内部使用 `par_iter()` 并行处理各行，无共享可变状态，
/// 可被上层跨文件的 `par_iter()` 安全调用（两级 rayon 嵌套并行）。
///
/// 结果在文件内去重：同一事务 ID 可能出现在数百条记录中，
/// 提前去重可显著减少跨文件合并时的中间数据量。
fn scan_log_file_for_matches(file_path: &str, cfg: &Config) -> Vec<CompactString> {
    use rayon::prelude::*;

    let Ok(parser) = LogParser::from_path(file_path) else {
        return Vec::new();
    };
    let filters = match &cfg.features.filters {
        Some(f) if f.has_transaction_filters() => f,
        _ => return Vec::new(),
    };

    // trxid 用 CompactString：数字字符串 ≤23 字节，内联存储，无堆分配。
    // 收集到 HashSet 实现文件内去重，rayon 支持并行 collect 到 std::HashSet。
    let trxids: std::collections::HashSet<CompactString> = parser
        .par_iter()
        .filter_map(std::result::Result::ok)
        .filter_map(|result| {
            let mut matched = false;

            if let Some(ind) = result.parse_indicators() {
                if filters
                    .indicators
                    .matches(ind.exec_id, ind.exectime, i64::from(ind.rowcount))
                {
                    matched = true;
                }
            }
            if !matched && filters.sql.has_filters() {
                matched = filters.sql.matches(result.body().as_ref());
            }
            if matched {
                let meta = result.parse_meta();
                Some(CompactString::from(meta.trxid.as_ref()))
            } else {
                None
            }
        })
        .collect();
    trxids.into_iter().collect()
}

fn scan_for_trxids_by_transaction_filters(
    log_files: &[std::path::PathBuf],
    cfg: &Config,
    jobs: usize,
) -> AHashSet<CompactString> {
    use rayon::prelude::*;

    eprintln!(
        "Pre-scanning {} files for transaction-level filters...",
        log_files.len()
    );

    // 使用与主流程相同的线程数（jobs），避免预扫描阶段无限制占用 CPU。
    // pool.install() 使内层 scan_log_file_for_matches 的 par_iter() 也在同一池内调度。
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .expect("failed to build pre-scan thread pool");

    let matched: Vec<CompactString> = pool.install(|| {
        log_files
            .par_iter()
            .flat_map(|file| scan_log_file_for_matches(&file.to_string_lossy(), cfg))
            .collect()
    });

    matched.into_iter().collect()
}

fn make_progress_bar(quiet: bool, interval_ms: u64) -> ProgressBar {
    if quiet {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} [{prefix}] {msg} | {human_pos} records @ {per_sec} [{elapsed_precise}]",
        )
        .expect("valid template")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    pb.enable_steady_tick(Duration::from_millis(interval_ms));
    pb
}

/// 将 N 个已处理的临时 CSV 文件按顺序拼接到最终输出路径。
/// 第一个文件保留 header；后续文件跳过第一行。
/// `append_to_existing`=true 时所有文件都跳过 header（目标文件已有 header）。
fn concat_csv_parts(
    parts: &[(PathBuf, usize)],
    output_path: &Path,
    overwrite: bool,
    append_to_existing: bool,
) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::BufReader;

    // 无任何 part（如 resume 模式下全部文件已跳过）时不触碰输出文件，
    // 避免 overwrite=true 把已有数据清空。
    if parts.is_empty() {
        return Ok(());
    }

    let file = if append_to_existing {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(output_path)?
    } else {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(overwrite)
            .open(output_path)?
    };
    let mut writer = std::io::BufWriter::with_capacity(16 * 1024 * 1024, file);

    for (idx, (part_path, _)) in parts.iter().enumerate() {
        let part_file = std::fs::File::open(part_path)?;
        let mut reader = BufReader::new(part_file);

        // 第一个 part（且非追加模式）保留 header；其余情况跳过 header 行
        let skip_header = idx > 0 || append_to_existing;
        if skip_header {
            // 用 Vec<u8> + read_until 而非 String + read_line：
            // 省去 UTF-8 验证，预分配避免 header 超 capacity 时的二次分配。
            let mut discard = Vec::with_capacity(256);
            std::io::BufRead::read_until(&mut reader, b'\n', &mut discard)?;
        }

        std::io::copy(&mut reader, &mut writer)?;
        std::fs::remove_file(part_path)?;
    }

    use std::io::Write as _;
    writer.flush()?;
    Ok(())
}

/// 并行 CSV 处理：每个文件独立跑在 rayon 线程上，各写一个临时 CSV，
/// 最终按文件原始顺序拼接成一个完整 CSV。
///
/// 返回：`(已处理文件列表, 跳过文件数)`，已处理列表顺序与 `log_files` 一致。
/// 适用条件：CSV 导出 + 多文件 + jobs > 1 + 无 limit。
fn process_csv_parallel(
    log_files: &[PathBuf],
    cfg: &Config,
    pipeline: &Pipeline,
    jobs: usize,
    pb: &ProgressBar,
    interrupted: &Arc<AtomicBool>,
    resume_state: Option<&crate::resume::ResumeState>,
    quiet: bool,
    do_normalize: bool,
    placeholder_override: Option<bool>,
) -> Result<(Vec<(PathBuf, usize)>, usize)> {
    use rayon::prelude::*;

    let csv_cfg = cfg
        .exporter
        .csv
        .as_ref()
        .expect("parallel CSV requires CSV exporter");
    let output_path = Path::new(&csv_cfg.file);
    let append_to_existing = csv_cfg.append && output_path.exists();

    // 临时目录与最终输出文件相邻，避免跨设备 copy；
    // 若父目录不可写（如 /dev/null），退回到系统临时目录。
    let parts_dir = {
        let stem = output_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let dir_name = format!(".{stem}_parts_{}", std::process::id());
        let preferred = output_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if let Some(parent) = output_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let candidate = preferred.join(&dir_name);
        if std::fs::create_dir_all(&candidate).is_ok() {
            candidate
        } else {
            let fallback = std::env::temp_dir().join(&dir_name);
            std::fs::create_dir_all(&fallback)?;
            fallback
        }
    };

    let total_files = log_files.len();

    // 构建独立线程池，避免干扰全局 rayon 池（预扫描阶段已用）
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .map_err(|e| Error::Io(std::io::Error::other(e)))?;

    // 每个任务返回 Some((orig_path, temp_path, count)) 或 None（跳过/中断）
    type TaskResult = Option<(PathBuf, PathBuf, usize)>;
    let results: Vec<Result<TaskResult>> = pool.install(|| {
        log_files
            .par_iter()
            .enumerate()
            .map(|(idx, file)| {
                if let Some(state) = resume_state {
                    if state.is_processed(file) {
                        if !quiet {
                            pb.println(format!(
                                "{} [{}/{}] {} — skipped (already processed)",
                                color::dim("⏭"),
                                idx + 1,
                                total_files,
                                file.display(),
                            ));
                        }
                        return Ok(None);
                    }
                }

                if interrupted.load(Ordering::Relaxed) {
                    return Ok(None);
                }

                let temp_path = parts_dir.join(format!("{idx:08}.csv"));
                let mut exporter = CsvExporter::new(&temp_path);
                exporter.normalize = do_normalize;
                let mut em = ExporterManager::from_csv(exporter);
                em.initialize()?;

                let mut params_buf = ParamBuffer::default();
                let mut ns_scratch = Vec::with_capacity(4096);

                let count = process_log_file(
                    &file.to_string_lossy(),
                    idx + 1,
                    total_files,
                    &mut em,
                    pipeline,
                    pb,
                    None,
                    interrupted,
                    do_normalize,
                    placeholder_override,
                    &mut params_buf,
                    &mut ns_scratch,
                    false, // 并行模式：不重置进度条，避免多线程互相重置计数
                )?;

                em.finalize()?;
                Ok(Some((file.clone(), temp_path, count)))
            })
            .collect()
    });

    // 收集成功的任务；遇到错误先清理再返回
    // (orig, temp, count) 三元组，保持 rayon 的原始文件顺序
    let mut parts_info: Vec<(PathBuf, PathBuf, usize)> = Vec::with_capacity(log_files.len());
    let mut first_err: Option<Error> = None;
    let mut skipped = 0usize;
    for result in results {
        match result {
            Ok(Some(p)) => parts_info.push(p),
            Ok(None) => skipped += 1,
            Err(e) if first_err.is_none() => first_err = Some(e),
            Err(_) => {}
        }
    }
    if let Some(e) = first_err {
        for (_, temp, _) in &parts_info {
            let _ = std::fs::remove_file(temp);
        }
        let _ = std::fs::remove_dir_all(&parts_dir);
        return Err(e);
    }

    // 拼接：只用 (temp_path, count) 传给 concat_csv_parts
    let parts_for_concat: Vec<(PathBuf, usize)> = parts_info
        .iter()
        .map(|(_, temp, count)| (temp.clone(), *count))
        .collect();
    let concat_result = concat_csv_parts(
        &parts_for_concat,
        output_path,
        csv_cfg.overwrite,
        append_to_existing,
    );
    // 无论拼接成功与否都清理临时目录，避免磁盘满等错误导致残留
    let _ = std::fs::remove_dir_all(&parts_dir);
    concat_result?;

    // 返回 (已处理文件列表, 跳过文件数)，供 handle_run 更新 resume state 及摘要行
    Ok((
        parts_info
            .into_iter()
            .map(|(orig, _, count)| (orig, count))
            .collect(),
        skipped,
    ))
}

pub fn handle_run(
    cfg: &Config,
    limit: Option<usize>,
    dry_run: bool,
    quiet: bool,
    interrupted: &Arc<AtomicBool>,
    progress_interval: u64,
    resume: bool,
    state_file_override: Option<&str>,
    jobs: usize,
) -> Result<()> {
    let total_start = Instant::now();
    let log_files = SqllogParser::new(&cfg.sqllog.path).log_files()?;
    if log_files.is_empty() {
        warn!("No log files found");
        return Ok(());
    }

    let state_path =
        std::path::PathBuf::from(state_file_override.unwrap_or(&cfg.resume.state_file));
    let mut resume_state = if resume {
        let state = crate::resume::ResumeState::load(&state_path);
        info!(
            "Resume mode: state file {}, {} files previously processed",
            state_path.display(),
            state.processed_count()
        );
        Some(state)
    } else {
        None
    };

    // 仅当有事务级过滤器时才克隆配置（避免常规路径的额外分配）
    let owned_cfg;
    let final_cfg: &Config = if cfg
        .features
        .filters
        .as_ref()
        .is_some_and(crate::features::FiltersFeature::has_transaction_filters)
    {
        let extra_trxids = scan_for_trxids_by_transaction_filters(&log_files, cfg, jobs);
        let mut tmp = cfg.clone();
        if let Some(f) = &mut tmp.features.filters {
            // into_iter() yields CompactString; merge_found_trxids 接受 Vec<CompactString>
            f.merge_found_trxids(extra_trxids.into_iter().collect());
        }
        owned_cfg = tmp;
        &owned_cfg
    } else {
        cfg
    };

    let pipeline = build_pipeline(final_cfg);

    let do_normalize = final_cfg
        .features
        .replace_parameters
        .as_ref()
        .is_none_or(|r| r.enable);
    let placeholder_override = final_cfg
        .features
        .replace_parameters
        .as_ref()
        .and_then(crate::features::ReplaceParametersConfig::placeholder_override);

    let pb = make_progress_bar(quiet, progress_interval);
    let mut total_records = 0usize;
    let mut skipped_files = 0usize;

    // 并行 CSV 路径：多文件 + 无 limit + CSV 导出器 + jobs > 1
    let use_parallel = !dry_run
        && jobs > 1
        && log_files.len() > 1
        && limit.is_none()
        && final_cfg.exporter.csv.is_some();

    if use_parallel {
        info!("Parsing and exporting SQL logs (parallel, {jobs} jobs)...");

        let (processed_files, parallel_skipped) = process_csv_parallel(
            &log_files,
            final_cfg,
            &pipeline,
            jobs,
            &pb,
            interrupted,
            resume_state.as_ref(),
            quiet,
            do_normalize,
            placeholder_override,
        )?;

        total_records = processed_files.iter().map(|(_, c)| *c).sum();
        skipped_files = parallel_skipped;

        // 更新断点续传状态（并行路径完成后统一写入）。
        // 若被中断则不写入：并行任务无法区分"完整处理"与"中途截断"，
        // 保守地不标记任何文件为已完成，与顺序路径行为一致。
        if !interrupted.load(Ordering::Relaxed) {
            if let Some(state) = &mut resume_state {
                for (file, count) in &processed_files {
                    state.mark_processed(file, *count as u64)?;
                }
                state.save(&state_path)?;
            }
        }
    } else {
        // 顺序路径
        let mut exporter_manager = if dry_run {
            ExporterManager::dry_run()
        } else {
            ExporterManager::from_config(final_cfg)?
        };
        exporter_manager.initialize()?;

        if dry_run {
            info!("Dry-run: parsing SQL logs without writing output...");
        } else {
            info!("Parsing and exporting SQL logs...");
        }

        // 跨文件复用分配：process_log_file 在每次调用时 clear() 而不是重建
        let mut params_buffer = ParamBuffer::default();
        // 预分配 1024 字节：避免首条参数化 SQL 触发初始堆分配
        let mut ns_scratch: Vec<u8> = Vec::with_capacity(4096);

        for (idx, log_file) in log_files.iter().enumerate() {
            if interrupted.load(Ordering::Relaxed) {
                break;
            }

            let remaining = limit.map(|l| l.saturating_sub(total_records));
            if remaining == Some(0) {
                break;
            }

            if let Some(state) = &resume_state {
                if state.is_processed(log_file) {
                    skipped_files += 1;
                    pb.println(format!(
                        "{} [{}/{}] {} — skipped (already processed)",
                        color::dim("⏭"),
                        idx + 1,
                        log_files.len(),
                        log_file.display(),
                    ));
                    continue;
                }
            }

            let processed = process_log_file(
                &log_file.to_string_lossy(),
                idx + 1,
                log_files.len(),
                &mut exporter_manager,
                &pipeline,
                &pb,
                remaining,
                interrupted,
                do_normalize,
                placeholder_override,
                &mut params_buffer,
                &mut ns_scratch,
                true, // 顺序模式：每个文件开始时重置进度条
            )?;

            if !dry_run {
                if let Some(state) = &mut resume_state {
                    state.mark_processed(log_file, processed as u64)?;
                    state.save(&state_path)?;
                }
            }

            total_records += processed;
            if limit.is_some_and(|l| total_records >= l) {
                break;
            }
        }

        exporter_manager.finalize()?;
        if !quiet {
            exporter_manager.log_stats();
        }
    }

    pb.finish_and_clear();

    if !quiet {
        let elapsed = total_start.elapsed().as_secs_f64();
        let mode_label = if dry_run {
            " [dry-run]"
        } else if use_parallel {
            " [parallel]"
        } else {
            ""
        };
        let skip_label = if skipped_files > 0 {
            format!(", {} skipped", color::dim(HumanCount(skipped_files as u64)))
        } else {
            String::new()
        };
        eprintln!(
            "\n{} SQL Log Export Task Completed{mode_label} in {elapsed:.2}s — {} records total{skip_label}",
            color::green("✓"),
            color::green(HumanCount(total_records as u64)),
        );
    }

    if interrupted.load(Ordering::Relaxed) {
        return Err(Error::Interrupted);
    }
    Ok(())
}

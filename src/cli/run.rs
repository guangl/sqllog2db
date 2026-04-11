use crate::color;
use crate::config::Config;
use crate::error::ParserError;
use crate::error::{Error, Result};
use crate::exporter::ExporterManager;
use crate::features::filters::RecordMeta;
use crate::features::replace_parameters::ParamBuffer;
use crate::features::{LogProcessor, Pipeline};
use crate::parser::SqllogParser;
use ahash::HashSet as AHashSet;
use compact_str::CompactString;
use dm_database_parser_sqllog::{LogParser, MetaParts};
use indicatif::{HumanCount, ProgressBar, ProgressStyle};
use log::{info, warn};
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
) -> Result<usize> {
    // 清除上一个文件留下的残余参数，同时复用已分配的 HashMap 容量。
    params_buffer.clear();

    let file_start = Instant::now();

    let file_name = std::path::Path::new(file_path).file_name().map_or_else(
        || file_path.to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    pb.set_prefix(format!("{file_index}/{total_files}"));
    pb.set_message(file_name);
    pb.reset();

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

fn scan_log_file_for_trxids(
    file_path: &str,
    cfg: &Config,
    remaining_exec_ids: &mut AHashSet<i64>,
    found_trxids: &mut AHashSet<CompactString>,
) {
    use rayon::prelude::*;

    let Ok(parser) = LogParser::from_path(file_path) else {
        return;
    };
    let filters = match &cfg.features.filters {
        Some(f) if f.has_transaction_filters() => f,
        _ => return,
    };

    // 并行扫描：各 CPU 核心独立处理文件分片，收集匹配的 (trxid, exec_id)
    // trxid 用 CompactString：数字字符串 ≤23 字节，内联存储，无堆分配。
    let matched: Vec<(CompactString, Option<i64>)> = parser
        .par_iter()
        .filter_map(std::result::Result::ok)
        .filter_map(|result| {
            let mut matched_exec_id: Option<i64> = None;
            let mut sql_matched = false;

            if let Some(ind) = result.parse_indicators() {
                if filters
                    .indicators
                    .matches(ind.exec_id, ind.exectime, i64::from(ind.rowcount))
                {
                    matched_exec_id = Some(ind.exec_id);
                }
            }
            if matched_exec_id.is_none() && filters.sql.has_filters() {
                sql_matched = filters.sql.matches(result.body().as_ref());
            }
            if matched_exec_id.is_some() || sql_matched {
                let meta = result.parse_meta();
                Some((CompactString::from(meta.trxid.as_ref()), matched_exec_id))
            } else {
                None
            }
        })
        .collect();

    for (trxid, exec_id) in matched {
        found_trxids.insert(trxid);
        if let Some(id) = exec_id {
            remaining_exec_ids.remove(&id);
        }
    }
}

fn scan_for_trxids_by_transaction_filters(
    log_files: &[std::path::PathBuf],
    cfg: &Config,
) -> AHashSet<CompactString> {
    let mut found_trxids = AHashSet::default();
    let mut remaining_exec_ids: AHashSet<i64> = cfg
        .features
        .filters
        .as_ref()
        .and_then(|f| f.indicators.exec_ids.as_deref())
        .unwrap_or_default()
        .iter()
        .copied()
        .collect();

    eprintln!(
        "Pre-scanning {} files for transaction-level filters...",
        log_files.len()
    );

    for log_file in log_files {
        scan_log_file_for_trxids(
            &log_file.to_string_lossy(),
            cfg,
            &mut remaining_exec_ids,
            &mut found_trxids,
        );
        if remaining_exec_ids.is_empty()
            && !cfg.features.filters.as_ref().is_some_and(|f| {
                f.indicators.min_runtime_ms.is_some()
                    || f.indicators.min_row_count.is_some()
                    || f.sql.has_filters()
            })
        {
            break;
        }
    }
    found_trxids
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

pub fn handle_run(
    cfg: &Config,
    limit: Option<usize>,
    dry_run: bool,
    quiet: bool,
    interrupted: &Arc<AtomicBool>,
    progress_interval: u64,
    resume: bool,
    state_file_override: Option<&str>,
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

    let mut final_cfg = cfg.clone();
    if cfg
        .features
        .filters
        .as_ref()
        .is_some_and(crate::features::FiltersFeature::has_transaction_filters)
    {
        let extra_trxids = scan_for_trxids_by_transaction_filters(&log_files, cfg);
        if let Some(f) = &mut final_cfg.features.filters {
            // into_iter() yields CompactString; merge_found_trxids 接受 Vec<CompactString>
            f.merge_found_trxids(extra_trxids.into_iter().collect());
        }
    }
    let final_cfg = &final_cfg;

    let pipeline = build_pipeline(final_cfg);
    let mut exporter_manager = if dry_run {
        ExporterManager::dry_run()
    } else {
        ExporterManager::from_config(final_cfg)?
    };
    exporter_manager.initialize()?;

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

    if dry_run {
        info!("Dry-run: parsing SQL logs without writing output...");
    } else {
        info!("Parsing and exporting SQL logs...");
    }

    let pb = make_progress_bar(quiet, progress_interval);
    let mut total_records = 0usize;
    let mut skipped_files = 0usize;
    // 跨文件复用 ParamBuffer 分配：process_log_file 在每次调用时 clear() 而不是重建，
    // 避免为每个日志文件重新触发 HashMap 的初始分配。
    let mut params_buffer = ParamBuffer::default();
    // 跨记录复用 normalized SQL 的输出缓冲，消除每条参数化 SQL 的 String 堆分配。
    let mut ns_scratch: Vec<u8> = Vec::new();

    for (idx, log_file) in log_files.iter().enumerate() {
        if interrupted.load(Ordering::Relaxed) {
            break;
        }

        let remaining = limit.map(|l| l.saturating_sub(total_records));
        if remaining == Some(0) {
            break;
        }

        // 断点续传：跳过已完整处理的文件
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
        )?;

        // 处理完成后立即持久化状态（dry-run 不更新）
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

    pb.finish_and_clear();

    exporter_manager.finalize()?;

    if !quiet {
        let elapsed = total_start.elapsed().as_secs_f64();
        let mode_label = if dry_run { " [dry-run]" } else { "" };
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
        exporter_manager.log_stats();
    }

    if interrupted.load(Ordering::Relaxed) {
        return Err(Error::Interrupted);
    }
    Ok(())
}

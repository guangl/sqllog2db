use crate::color;
use crate::config::Config;
use crate::error::ParserError;
use crate::error::{Error, Result};
use crate::exporter::ExporterManager;
use crate::features::{LogProcessor, Pipeline};
use crate::parser::SqllogParser;
use dm_database_parser_sqllog::LogParser;
use indicatif::{HumanCount, ProgressBar, ProgressStyle};
use log::{info, warn};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// 构建处理器管线
fn build_pipeline(cfg: &Config) -> Pipeline {
    let mut pipeline = Pipeline::new();

    if let Some(f) = &cfg.features.filters {
        if f.has_filters() {
            pipeline.add(Box::new(FilterProcessor { filter: f.clone() }));
        }
    }

    pipeline
}

#[derive(Debug)]
struct FilterProcessor {
    filter: crate::features::FiltersFeature,
}

impl LogProcessor for FilterProcessor {
    fn process(&self, record: &dm_database_parser_sqllog::Sqllog) -> bool {
        let meta = record.parse_meta();
        self.filter.should_keep(
            record.ts.as_ref(),
            &meta.trxid,
            &meta.client_ip,
            &meta.sess_id,
            &meta.thrd_id,
            &meta.username,
            &meta.statement,
            &meta.appname,
            record.tag.as_deref(),
        )
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
) -> Result<usize> {
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
    let mut batch = Vec::with_capacity(5000);

    let mut params_buffer = std::collections::HashMap::new();
    let mut batch_normalized: Vec<Option<String>> = Vec::with_capacity(5000);

    macro_rules! flush_batch {
        () => {
            if !batch.is_empty() {
                let batch_len = batch.len();
                records_in_file += batch_len;
                pb.inc(batch_len as u64);

                if do_normalize {
                    exporter_manager.export_batch_with_normalized(&batch, &batch_normalized)?;
                    batch_normalized.clear();
                } else {
                    exporter_manager.export_batch(&batch)?;
                }

                batch.clear();
            }
        };
    }

    'outer: for result in parser.iter() {
        // batch 结束后检查中断信号（Relaxed 原子读，无额外开销）
        if interrupted.load(Ordering::Relaxed) {
            flush_batch!();
            break 'outer;
        }

        match result {
            Ok(record) => {
                let ns = if do_normalize {
                    crate::features::compute_normalized(
                        &record,
                        &mut params_buffer,
                        placeholder_override,
                    )
                } else {
                    None
                };

                let passes = pipeline.is_empty() || pipeline.run(&record);
                if passes {
                    // 检查是否即将超出本文件的剩余配额
                    if let Some(remaining) = limit {
                        if records_in_file + batch.len() + 1 > remaining {
                            flush_batch!();
                            break 'outer;
                        }
                    }

                    if do_normalize {
                        batch_normalized.push(ns);
                    }
                    batch.push(record);
                    if batch.len() >= 5000 {
                        flush_batch!();
                    }
                }
            }
            Err(e) => {
                errors_in_file += 1;
                flush_batch!();
                log::trace!("{file_path} | {e:?}");
            }
        }
    }

    flush_batch!();

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
    remaining_exec_ids: &mut HashSet<i64>,
    found_trxids: &mut HashSet<String>,
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
    let matched: Vec<(String, Option<i64>)> = parser
        .par_iter()
        .filter_map(std::result::Result::ok)
        .filter_map(|result| {
            let mut matched_exec_id: Option<i64> = None;
            let mut sql_matched = false;

            if let Some(ind) = result.parse_indicators() {
                #[allow(clippy::cast_possible_truncation)]
                let runtime_ms = ind.exectime.round() as i64;
                if filters
                    .indicators
                    .matches(ind.exec_id, runtime_ms, i64::from(ind.rowcount))
                {
                    matched_exec_id = Some(ind.exec_id);
                }
            }
            if matched_exec_id.is_none() && filters.sql.has_filters() {
                sql_matched = filters.sql.matches(result.body().as_ref());
            }
            if matched_exec_id.is_some() || sql_matched {
                let meta = result.parse_meta();
                Some((meta.trxid.to_string(), matched_exec_id))
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
) -> HashSet<String> {
    let mut found_trxids = HashSet::new();
    let mut remaining_exec_ids: HashSet<i64> = cfg
        .features
        .filters
        .as_ref()
        .and_then(|f| f.indicators.exec_ids.clone())
        .unwrap_or_default()
        .into_iter()
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

fn make_progress_bar(quiet: bool) -> ProgressBar {
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
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

pub fn handle_run(
    cfg: &Config,
    limit: Option<usize>,
    dry_run: bool,
    quiet: bool,
    interrupted: &Arc<AtomicBool>,
) -> Result<()> {
    let total_start = Instant::now();
    let log_files = SqllogParser::new(&cfg.sqllog.directory).log_files()?;
    if log_files.is_empty() {
        warn!("No log files found");
        return Ok(());
    }

    let mut final_cfg = cfg.clone();
    if cfg
        .features
        .filters
        .as_ref()
        .is_some_and(crate::features::FiltersFeature::has_transaction_filters)
    {
        let extra_trxids = scan_for_trxids_by_transaction_filters(&log_files, cfg);
        if let Some(f) = &mut final_cfg.features.filters {
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

    let pb = make_progress_bar(quiet);
    let mut total_records = 0usize;

    for (idx, log_file) in log_files.iter().enumerate() {
        if interrupted.load(Ordering::Relaxed) {
            break;
        }

        let remaining = limit.map(|l| l.saturating_sub(total_records));
        if remaining == Some(0) {
            break;
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
        )?;

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
        eprintln!(
            "\n{} SQL Log Export Task Completed{mode_label} in {elapsed:.2}s — {} records total",
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

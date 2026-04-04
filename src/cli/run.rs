use crate::config::Config;
use crate::error::ParserError;
use crate::error::{Error, Result};
use crate::error_logger::ErrorLogger;
use crate::exporter::ExporterManager;
use crate::features::Pipeline;
use crate::parser::SqllogParser;
use dm_database_parser_sqllog::LogParser;
use log::{info, warn};
use std::time::Instant;

#[cfg(feature = "filters")]
use std::collections::HashSet;

#[cfg(feature = "filters")]
use crate::features::LogProcessor;

/// 构建处理器管线
fn build_pipeline(cfg: &Config) -> Pipeline {
    #[allow(unused_mut)]
    let mut pipeline = Pipeline::new();

    #[cfg(feature = "filters")]
    if let Some(f) = &cfg.features.filters {
        if f.has_filters() {
            pipeline.add(Box::new(FilterProcessor { filter: f.clone() }));
        }
    }

    #[cfg(not(feature = "filters"))]
    let _ = cfg;

    pipeline
}

#[cfg(feature = "filters")]
#[derive(Debug)]
struct FilterProcessor {
    filter: crate::features::FiltersFeature,
}

#[cfg(feature = "filters")]
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

fn process_log_file(
    file_path: &str,
    file_index: usize,
    total_files: usize,
    exporter_manager: &mut ExporterManager,
    error_logger: &mut ErrorLogger,
    pipeline: &Pipeline,
    #[cfg(feature = "replace_parameters")] do_normalize: bool,
    #[cfg(feature = "replace_parameters")] placeholder_override: Option<bool>,
) -> Result<()> {
    let file_start = Instant::now();
    eprintln!("[{file_index}/{total_files}] Processing: {file_path}");

    let parser = LogParser::from_path(file_path).map_err(|e| {
        Error::Parser(ParserError::InvalidPath {
            path: file_path.into(),
            reason: format!("{e}"),
        })
    })?;

    let mut records_in_file = 0usize;
    let mut errors_in_file = 0usize;
    let mut batch = Vec::with_capacity(5000);

    #[cfg(feature = "replace_parameters")]
    let mut params_buffer = std::collections::HashMap::new();
    #[cfg(feature = "replace_parameters")]
    let mut batch_normalized: Vec<Option<String>> = Vec::with_capacity(5000);

    macro_rules! flush_batch {
        () => {
            if !batch.is_empty() {
                records_in_file += batch.len();
                #[cfg(feature = "replace_parameters")]
                if do_normalize {
                    exporter_manager.export_batch_with_normalized(&batch, &batch_normalized)?;
                    batch_normalized.clear();
                } else {
                    exporter_manager.export_batch(&batch)?;
                }
                #[cfg(not(feature = "replace_parameters"))]
                exporter_manager.export_batch(&batch)?;
                batch.clear();
            }
        };
    }

    for result in parser.iter() {
        match result {
            Ok(record) => {
                #[cfg(feature = "replace_parameters")]
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
                    #[cfg(feature = "replace_parameters")]
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
                if let Err(log_err) = error_logger.log_parse_error(file_path, &e) {
                    warn!("Failed to record parse error: {log_err}");
                }
            }
        }
    }

    flush_batch!();

    info!(
        "File {}: {} records, {} errors, total {:.2}s",
        file_path,
        records_in_file,
        errors_in_file,
        file_start.elapsed().as_secs_f64()
    );

    Ok(())
}

#[cfg(feature = "filters")]
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

#[cfg(feature = "filters")]
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

pub fn handle_run(cfg: &Config) -> Result<()> {
    let total_start = Instant::now();
    let log_files = SqllogParser::new(&cfg.sqllog.directory).log_files()?;
    if log_files.is_empty() {
        warn!("No log files found");
        return Ok(());
    }

    #[cfg(feature = "filters")]
    let mut final_cfg = cfg.clone();
    #[cfg(feature = "filters")]
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
    #[cfg(feature = "filters")]
    let final_cfg = &final_cfg;
    #[cfg(not(feature = "filters"))]
    let final_cfg = cfg;

    let pipeline = build_pipeline(final_cfg);
    let mut exporter_manager = ExporterManager::from_config(final_cfg)?;
    let mut error_logger = ErrorLogger::new(&final_cfg.error.file)?;
    exporter_manager.initialize()?;

    #[cfg(feature = "replace_parameters")]
    let do_normalize = final_cfg
        .features
        .replace_parameters
        .as_ref()
        .is_none_or(|r| r.enable);

    #[cfg(feature = "replace_parameters")]
    let placeholder_override = final_cfg
        .features
        .replace_parameters
        .as_ref()
        .and_then(crate::features::ReplaceParametersConfig::placeholder_override);

    info!("Parsing and exporting SQL logs...");
    for (idx, log_file) in log_files.iter().enumerate() {
        process_log_file(
            &log_file.to_string_lossy(),
            idx + 1,
            log_files.len(),
            &mut exporter_manager,
            &mut error_logger,
            &pipeline,
            #[cfg(feature = "replace_parameters")]
            do_normalize,
            #[cfg(feature = "replace_parameters")]
            placeholder_override,
        )?;
    }

    exporter_manager.finalize()?;
    error_logger.finalize()?;

    eprintln!(
        "\n✓ SQL Log Export Task Completed in {:.2}s",
        total_start.elapsed().as_secs_f64()
    );
    exporter_manager.log_stats();
    Ok(())
}

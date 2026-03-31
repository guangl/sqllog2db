use crate::error::{Error, Result};
use crate::error_logger::ErrorLogger;
use crate::exporter::ExporterManager;
use crate::features::FiltersFeature;
use crate::parser::SqllogParser;
use crate::{config::Config, error::ParserError};
use dm_database_parser_sqllog::LogParser;
use log::{info, warn};
use std::collections::HashSet;
use std::time::Instant;

/// 处理单个日志文件，带进度统计
fn process_log_file(
    file_path: &str,
    file_index: usize,
    total_files: usize,
    exporter_manager: &mut ExporterManager,
    error_logger: &mut ErrorLogger,
    cfg: &Config,
) -> Result<()> {
    let file_start = Instant::now();
    eprintln!("[{file_index}/{total_files}] Processing: {file_path}");
    info!("Processing file {file_index}/{total_files}: {file_path}");

    let parser = LogParser::from_path(file_path).map_err(|e| {
        Error::Parser(ParserError::InvalidPath {
            path: file_path.into(),
            reason: format!("{e}"),
        })
    })?;

    // 内存优化：使用更小的批次大小
    let mut batch = Vec::with_capacity(1000);
    let mut records_in_file = 0;
    let mut errors_in_file = 0;

    for result in parser.iter() {
        match result {
            Ok(record) => {
                // 应用过滤器
                let meta = record.parse_meta();
                if !cfg
                    .features
                    .filters
                    .as_ref()
                    .is_none_or(|f: &FiltersFeature| {
                        f.should_keep(
                            record.ts.as_ref(),
                            &meta.trxid,
                            &meta.client_ip,
                            &meta.sess_id,
                            &meta.thrd_id,
                            &meta.username,
                            &meta.statement,
                            &meta.appname,
                        )
                    })
                {
                    continue;
                }

                records_in_file += 1;
                batch.push(record);
                if batch.len() >= 1000 {
                    exporter_manager.export_batch(&batch)?;
                    batch.clear();
                    eprintln!("  [Progress] {records_in_file} records processed");
                }
            }
            Err(e) => {
                errors_in_file += 1;
                if !batch.is_empty() {
                    exporter_manager.export_batch(&batch)?;
                    batch.clear();
                }
                if let Err(log_err) = error_logger.log_parse_error(file_path, &e) {
                    warn!("Failed to record parse error: {log_err}");
                }
            }
        }
    }

    if !batch.is_empty() {
        exporter_manager.export_batch(&batch)?;
    }

    let file_elapsed = file_start.elapsed();
    let file_secs = file_elapsed.as_secs_f64();

    info!(
        "File {file_path} processed: {records_in_file} records, {errors_in_file} errors, {file_secs:.2}s"
    );

    Ok(())
}

/// 预扫描单个日志文件以寻找匹配过滤条件的事务 ID (Transaction-level)
fn scan_log_file_for_trxids(
    file_path: &str,
    cfg: &Config,
    remaining_exec_ids: &mut HashSet<i64>,
    found_trxids: &mut HashSet<String>,
) {
    let Ok(parser) = LogParser::from_path(file_path) else {
        return;
    };

    let filters = match &cfg.features.filters {
        Some(f) if f.enable => f,
        _ => return,
    };

    for result in parser.iter().flatten() {
        let mut matched = false;

        // 1. 检查指标过滤器 (Indicators)
        if let Some(ind) = result.parse_indicators() {
            // execute_time 已经是毫秒 (f32)
            #[allow(clippy::cast_possible_truncation)]
            let runtime_ms = ind.execute_time.round() as i64;

            if filters
                .indicators
                .matches(ind.execute_id, runtime_ms, i64::from(ind.row_count))
            {
                matched = true;
                remaining_exec_ids.remove(&ind.execute_id);
            }
        }

        // 2. 检查 SQL 过滤器
        if !matched && filters.sql.has_filters() && filters.sql.matches(result.body().as_ref()) {
            matched = true;
        }

        if matched {
            let meta = result.parse_meta();
            found_trxids.insert(meta.trxid.to_string());

            // 如果已经找齐了所有指定的 exec_id，且没有其他非确定性的过滤器，可以提前结束当前文件
            if remaining_exec_ids.is_empty()
                && filters.indicators.min_runtime_ms.is_none()
                && filters.indicators.min_row_count.is_none()
                && !filters.sql.has_filters()
            {
                break;
            }
        }
    }
}

/// 预扫描所有日志文件
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

    let total = log_files.len();
    eprintln!("Pre-scanning {total} files for transaction-level filters...");

    for (idx, log_file) in log_files.iter().enumerate() {
        let file_path_str = log_file.to_string_lossy();
        if idx % 10 == 0 || idx + 1 == total {
            eprintln!(
                "  [{}/{total}] Scanning: {file_path_str} (found so far: {})",
                idx + 1,
                found_trxids.len()
            );
        }

        scan_log_file_for_trxids(
            &file_path_str,
            cfg,
            &mut remaining_exec_ids,
            &mut found_trxids,
        );

        // 提前结束所有文件的预扫描 (仅当所有 exec_id 找齐且无其他模糊匹配器时)
        if remaining_exec_ids.is_empty()
            && !cfg.features.filters.as_ref().is_some_and(|f| {
                f.indicators.min_runtime_ms.is_some() || f.indicators.min_row_count.is_some()
            })
            && !cfg
                .features
                .filters
                .as_ref()
                .is_some_and(|f| f.sql.has_filters())
        {
            eprintln!("  [Done] All criteria satisfied.");
            break;
        }
    }

    if !remaining_exec_ids.is_empty() {
        warn!("Could not find trxid for some exec_ids: {remaining_exec_ids:?}");
    }

    eprintln!(
        "Found {} unique trxid(s) matching transaction-level criteria",
        found_trxids.len()
    );
    found_trxids
}

/// 运行日志导出任务（单线程、单导出器架构）
pub fn handle_run(cfg: &Config) -> Result<()> {
    let total_start = Instant::now();
    info!("Starting SQL log export task");

    let parser = SqllogParser::new(cfg.sqllog.directory());
    let log_files = parser.log_files()?;

    if log_files.is_empty() {
        warn!("No log files found");
        return Ok(());
    }

    info!("Found {} log file(s)", log_files.len());

    // 第三步：如果启用了事务级过滤 (indicators/sql)，进行预扫描
    let mut final_cfg = cfg.clone();
    let has_transaction_filters = cfg
        .features
        .filters
        .as_ref()
        .is_some_and(crate::config::FiltersFeature::has_transaction_filters);

    if has_transaction_filters {
        let extra_trxids = scan_for_trxids_by_transaction_filters(&log_files, cfg);
        if let Some(f) = &mut final_cfg.features.filters {
            f.merge_found_trxids(extra_trxids.into_iter().collect());
        }
    }

    // 第四步：创建导出器管理器
    let mut exporter_manager = ExporterManager::from_config(&final_cfg)?;
    info!("Using exporter: {}", exporter_manager.name());

    // 第五步：创建错误日志记录器
    let mut error_logger = ErrorLogger::new(final_cfg.error.file())?;

    // 第六步：初始化导出器
    info!("Initializing exporters...");
    exporter_manager.initialize()?;

    // 第七步：解析 SQL 日志（流式）并导出
    info!("Parsing and exporting SQL logs (streaming)...");

    for (idx, log_file) in log_files.iter().enumerate() {
        let file_path_str = log_file.to_string_lossy().to_string();
        process_log_file(
            &file_path_str,
            idx + 1,
            log_files.len(),
            &mut exporter_manager,
            &mut error_logger,
            &final_cfg,
        )?;
    }

    // 第八步：完成导出
    info!("Export finished...");
    exporter_manager.finalize()?;
    error_logger.finalize()?;

    // 计算统计信息
    let total_elapsed = total_start.elapsed();
    exporter_manager.log_stats();

    eprintln!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("✓ SQL Log Export Task Completed");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Exporter:  {}", exporter_manager.name());
    let elapsed_secs = total_elapsed.as_secs_f64();
    eprintln!("  Elapsed:   {elapsed_secs:.2} seconds");

    if let Some(stats) = exporter_manager.stats() {
        let elapsed_millis = total_elapsed.as_millis();
        let throughput = if elapsed_millis > 0 {
            (stats.exported as u128 * 1_000) / elapsed_millis
        } else {
            0
        };
        eprintln!("  Records:   {}", stats.exported);
        eprintln!("  Throughput: {throughput} records/sec");
    }
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}

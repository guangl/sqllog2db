use crate::error::{Error, Result};
use crate::error_logger::ErrorLogger;
use crate::exporter::ExporterManager;
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

    // 内存优化：使用更小的批次大小（1000 而不是 5000）
    // 这样可以更及时地释放内存，降低峰值
    let mut batch = Vec::with_capacity(1000);
    let mut records_in_file = 0;
    let mut errors_in_file = 0;

    for result in parser.iter() {
        match result {
            Ok(record) => {
                // 应用过滤器 (trxid, client_ip)
                let meta = record.parse_meta();
                let should_keep = cfg
                    .features
                    .filters
                    .as_ref()
                    .is_none_or(|f| f.should_keep_meta(&meta.trxid, &meta.client_ip));

                if !should_keep {
                    continue;
                }

                batch.push(record);
                records_in_file += 1;
                if batch.len() >= 1000 {
                    exporter_manager.export_batch(&batch)?;
                    batch.clear();
                    // 每 1000 条记录输出一次进度
                    eprintln!("  [Progress] {records_in_file} records processed");
                }
            }
            Err(e) => {
                errors_in_file += 1;
                // 如果有未处理的批次，先导出
                if !batch.is_empty() {
                    exporter_manager.export_batch(&batch)?;
                    batch.clear();
                }
                // 记录解析错误
                if let Err(log_err) = error_logger.log_parse_error(file_path, &e) {
                    warn!("Failed to record parse error: {log_err}");
                }
            }
        }
    }

    // 处理剩余的批次
    if !batch.is_empty() {
        exporter_manager.export_batch(&batch)?;
    }

    let file_elapsed = file_start.elapsed();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_lossless
    )]
    let throughput = if file_elapsed.as_secs_f64() > 0.0 {
        (records_in_file as f64 / file_elapsed.as_secs_f64()).round() as usize
    } else {
        0
    };

    let file_secs = file_elapsed.as_secs_f64();
    eprintln!(
        "  [Complete] {records_in_file} records, {errors_in_file} errors, {file_secs:.2}s, {throughput} records/sec"
    );
    info!(
        "File {file_path} processed: {records_in_file} records, {errors_in_file} errors, {file_secs:.2}s"
    );

    Ok(())
}

/// 预扫描单个日志文件以寻找匹配执行 ID 的事务 ID
fn scan_log_file_for_trxids(
    file_path: &str,
    remaining_exec_ids: &mut HashSet<i64>,
    found_trxids: &mut HashSet<String>,
) {
    let Ok(parser) = LogParser::from_path(file_path) else {
        return;
    };

    for result in parser.iter().flatten() {
        if let Some(ind) = result.parse_indicators() {
            if remaining_exec_ids.contains(&ind.execute_id) {
                let meta = result.parse_meta();
                found_trxids.insert(meta.trxid.to_string());
                remaining_exec_ids.remove(&ind.execute_id);

                // 如果当前文件已经找齐了所有需要的 exec_id，提前结束当前文件解析
                if remaining_exec_ids.is_empty() {
                    break;
                }
            }
        }
    }
}

/// 预扫描所有日志文件
fn scan_for_trxids_by_exec_ids(log_files: &[std::path::PathBuf], cfg: &Config) -> HashSet<String> {
    let mut found_trxids = HashSet::new();
    let mut remaining_exec_ids: HashSet<i64> = cfg
        .features
        .filters
        .as_ref()
        .and_then(|f| f.exec_ids.clone())
        .unwrap_or_default()
        .into_iter()
        .collect();

    if remaining_exec_ids.is_empty() {
        return found_trxids;
    }

    let total = log_files.len();
    eprintln!(
        "Pre-scanning {total} files for {} exec_id(s)...",
        remaining_exec_ids.len()
    );

    for (idx, log_file) in log_files.iter().enumerate() {
        let file_path_str = log_file.to_string_lossy();
        if idx % 10 == 0 || idx + 1 == total {
            eprintln!(
                "  [{}/{total}] Scanning: {file_path_str} (remaining: {})",
                idx + 1,
                remaining_exec_ids.len()
            );
        }

        scan_log_file_for_trxids(&file_path_str, &mut remaining_exec_ids, &mut found_trxids);

        // 如果所有执行 ID 都已找到对应的事务 ID，则提前结束所有文件的预扫描
        if remaining_exec_ids.is_empty() {
            eprintln!("  [Done] All target exec_ids found.");
            break;
        }
    }

    if !remaining_exec_ids.is_empty() {
        warn!("Could not find trxid for some exec_ids: {remaining_exec_ids:?}");
    }

    eprintln!(
        "Found {} unique trxid(s) matching exec_id filters",
        found_trxids.len()
    );
    found_trxids
}

/// 运行日志导出任务（单线程、单导出器架构）
pub fn handle_run(cfg: &Config) -> Result<()> {
    // 记录总体开始时间
    let total_start = Instant::now();

    info!("Starting SQL log export task");

    // 第一步：创建 SQL 日志解析器
    let parser = SqllogParser::new(cfg.sqllog.directory());
    info!("SQL log input directory: {}", parser.path().display());

    // 第二步：获取所有日志文件
    let log_files = parser.log_files()?;

    if log_files.is_empty() {
        warn!("No log files found");
        return Ok(());
    }

    info!("Found {} log file(s)", log_files.len());

    // 第三步：如果启用了执行 ID 过滤，进行预扫描
    let mut final_cfg = cfg.clone();
    let has_exec_filters = cfg
        .features
        .filters
        .as_ref()
        .is_some_and(crate::config::FiltersFeature::has_exec_id_filters);

    if has_exec_filters {
        let extra_trxids = scan_for_trxids_by_exec_ids(&log_files, cfg);
        if let Some(f) = &mut final_cfg.features.filters {
            f.merge_trxids(extra_trxids.into_iter().collect());
        }
    }

    // 第四步：创建导出器管理器（单个导出器）
    let mut exporter_manager = ExporterManager::from_config(&final_cfg)?;
    info!("Using exporter: {}", exporter_manager.name());

    // 第五步：创建错误日志记录器
    let mut error_logger = ErrorLogger::new(final_cfg.error.file())?;

    // 第六步：初始化导出器
    info!("Initializing exporters...");
    exporter_manager.initialize()?;

    // 第七步：解析 SQL 日志（流式）并导出
    info!("Parsing and exporting SQL logs (streaming)...");

    // 处理所有日志文件
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

    // 第九步：完成错误日志记录
    error_logger.finalize()?;

    // 计算总耗时
    let total_elapsed = total_start.elapsed();

    // 展示统计信息
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

    info!("✓ SQL log export task completed!");

    Ok(())
}

use crate::error::{Error, Result};
use crate::error_logger::ErrorLogger;
use crate::exporter::ExporterManager;
use crate::parser::SqllogParser;
use crate::{config::Config, error::ParserError};
use dm_database_parser_sqllog::LogParser;
use log::{info, warn};
use std::time::Instant;

/// 处理单个日志文件
fn process_log_file(
    file_path: &str,
    exporter_manager: &mut ExporterManager,
    error_logger: &mut ErrorLogger,
) -> Result<()> {
    info!("Processing file: {file_path}");

    let parser = LogParser::from_path(file_path).map_err(|e| {
        Error::Parser(ParserError::InvalidPath {
            path: file_path.into(),
            reason: format!("{e}"),
        })
    })?;

    // 内存优化：使用更小的批次大小（1000 而不是 5000）
    // 这样可以更及时地释放内存，降低峰值
    let mut batch = Vec::with_capacity(1000);
    for result in parser.iter() {
        match result {
            Ok(record) => {
                batch.push(record);
                if batch.len() >= 1000 {
                    exporter_manager.export_batch(&batch)?;
                    batch.clear();
                }
            }
            Err(e) => {
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

    Ok(())
}

/// 运行日志导出任务（单线程、单导出器架构）
pub fn handle_run(cfg: &Config) -> Result<()> {
    // 记录总体开始时间
    let total_start = Instant::now();

    info!("Starting SQL log export task");

    // 第一步：创建 SQL 日志解析器
    let parser = SqllogParser::new(cfg.sqllog.directory());
    info!("SQL log input directory: {}", parser.path().display());

    // 第二步：创建导出器管理器（单个导出器）
    let mut exporter_manager = ExporterManager::from_config(cfg)?;
    info!("Using exporter: {}", exporter_manager.name());

    // 第三步：创建错误日志记录器
    let mut error_logger = ErrorLogger::new(cfg.error.file())?;

    // 第四步：初始化导出器
    info!("Initializing exporters...");
    exporter_manager.initialize()?;

    // 第五步：解析 SQL 日志（流式）并导出
    info!("Parsing and exporting SQL logs (streaming)...");

    // 获取所有日志文件
    let log_files = parser.log_files()?;

    if log_files.is_empty() {
        warn!("No log files found");
        exporter_manager.finalize()?;
        error_logger.finalize()?;
        return Ok(());
    }

    info!("Found {} log file(s)", log_files.len());

    // 处理所有日志文件
    for (idx, log_file) in log_files.iter().enumerate() {
        let file_path_str = log_file.to_string_lossy().to_string();
        info!(
            "Processing file {}/{}: {}",
            idx + 1,
            log_files.len(),
            log_file.display()
        );
        process_log_file(&file_path_str, &mut exporter_manager, &mut error_logger)?;
    }

    // 第六步：完成导出
    info!("Export finished...");
    exporter_manager.finalize()?;

    // 第七步：完成错误日志记录
    error_logger.finalize()?;

    // 计算总耗时
    let total_elapsed = total_start.elapsed().as_secs_f64();

    // 展示统计信息
    exporter_manager.log_stats();

    eprintln!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("✓ SQL Log Export Task Completed");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Exporter:  {}", exporter_manager.name());
    eprintln!("  Elapsed:   {total_elapsed:.3} seconds");
    if let Some(stats) = exporter_manager.stats() {
        if total_elapsed > 0.0 {
            let throughput = stats.exported as f64 / total_elapsed;
            eprintln!("  Records:   {}", stats.exported);
            eprintln!("  Throughput: {throughput:.0} records/sec");
        }
    }
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    info!("✓ SQL log export task completed!");

    Ok(())
}

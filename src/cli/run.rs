use crate::config::Config;
use crate::error::Result;
use crate::error_logger::ErrorLogger;
use crate::exporter::ExporterManager;
use crate::parser::SqllogParser;
use log::{info, warn};
use std::time::Instant;

/// 批处理统计信息
struct BatchStats {
    total: usize,
    error_count: usize,
}

impl BatchStats {
    fn new() -> Self {
        Self {
            total: 0,
            error_count: 0,
        }
    }

    fn record_success_with_count(&mut self, count: usize) {
        self.total += count;
    }

    fn record_success(&mut self) {
        self.total += 1;
    }

    fn record_error_with_count(&mut self, count: usize) {
        self.error_count += count;
    }

    fn record_error(&mut self) {
        self.error_count += 1;
    }
}

/// 处理单个日志文件
fn process_log_file(
    file_path: &str,
    exporter_manager: &mut ExporterManager,
    error_logger: &mut ErrorLogger,
    stats: &mut BatchStats,
) -> Result<()> {
    info!("Processing file: {}", file_path);

    match dm_database_parser_sqllog::parse_records_from_file(file_path) {
        Ok((records, parse_errors)) => {
            // 统计解析时间
            let insert_start = Instant::now();
            exporter_manager.export_batch(&records)?;
            let insert_elapsed = insert_start.elapsed().as_secs_f64();
            info!("插入耗时: {:.3} 秒", insert_elapsed);

            stats.record_success_with_count(records.len());
            stats.record_error_with_count(parse_errors.len());
        }
        Err(e) => {
            stats.record_error();
            warn!("Failed to open file {}: {}", file_path, e);
            if let Err(log_err) = error_logger.log_parse_error(file_path, &e) {
                warn!("Failed to record file error: {}", log_err);
            }
        }
    }

    Ok(())
}

/// 运行日志导出任务（单线程、单导出器架构）
pub fn handle_run(cfg: &Config) -> Result<()> {
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
    let mut stats = BatchStats::new();

    // 获取所有日志文件
    let log_files = parser.log_files()?;

    if log_files.is_empty() {
        warn!("No log files found");
        exporter_manager.finalize()?;
        return Ok(());
    }

    info!("Found {} log files", log_files.len());

    // 统计解析时间
    let total_start = Instant::now();
    for log_file in log_files {
        let file_path_str = log_file.to_string_lossy().to_string();
        process_log_file(
            &file_path_str,
            &mut exporter_manager,
            &mut error_logger,
            &mut stats,
        )?;
    }
    let total_elapsed = total_start.elapsed().as_secs_f64();

    if stats.total == 0 {
        warn!("No SQL log records parsed");
        exporter_manager.finalize()?;
        return Ok(());
    }

    info!("Successfully parsed {} SQL log records", stats.total);
    if stats.error_count > 0 {
        warn!("Encountered {} errors during parsing", stats.error_count);
    }

    // 第六步：完成导出
    info!("Export finished...");
    exporter_manager.finalize()?;

    // 展示统计信息
    exporter_manager.log_stats();

    info!("✓ SQL log export task completed!");
    info!("  - 解析记录数: {}", stats.total);
    info!("  - 解析错误数: {}", stats.error_count);
    info!("  - 导出器: {}", exporter_manager.name());
    info!("  - 解析耗时: {:.3} 秒", total_elapsed);

    Ok(())
}

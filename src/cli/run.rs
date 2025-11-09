use tracing::{debug, info, warn};

use crate::config::Config;
use crate::error::Result;
use crate::error_logger::ErrorLogger;
use crate::exporter::ExporterManager;
use crate::parser::SqllogParser;
use dm_database_parser_sqllog::Sqllog;

/// 运行日志导出任务
pub fn handle_run(cfg: &Config) -> Result<()> {
    info!("开始运行 SQL 日志导出任务");

    // 第一步：创建 SQL 日志解析器
    let parser = SqllogParser::new(cfg.sqllog.path(), cfg.sqllog.thread_count());
    info!("SQL 日志路径: {}", parser.path().display());

    // 第二步：创建导出器管理器
    let mut exporter_manager = ExporterManager::from_config(cfg)?;
    info!("已配置 {} 个导出器", exporter_manager.count());

    // 第三步：创建错误日志记录器
    let mut error_logger = ErrorLogger::new(cfg.error.path())?;

    // 第四步：初始化所有导出器
    info!("初始化导出器...");
    exporter_manager.initialize()?;

    // 第五步：解析 SQL 日志（流式）并导出
    info!("解析并导出 SQL 日志（流式）...");
    let batch_size = exporter_manager.batch_size();
    let mut total: usize = 0;
    let mut local_buf: Vec<Sqllog> = Vec::new();
    let log_every = if batch_size > 0 {
        batch_size.max(1000)
    } else {
        1000
    };

    parser.parse_with(
        |record| {
            total += 1;
            if batch_size > 0 {
                // 累积到本地缓冲，分批调用 export_batch（避免一次性占用内存）
                local_buf.push(record.clone());
                if local_buf.len() >= batch_size {
                    exporter_manager.export_batch(&local_buf)?;
                    local_buf.clear();
                }
            } else {
                // 逐条调用，各导出器内部（如 CSV/JSONL）会在 finalize 统一写出
                exporter_manager.export(record)?;
            }

            if total % log_every == 0 {
                info!("已解析并分发 {} 条记录...", total);
            }
            Ok(())
        },
        |file_path, error| {
            // 记录解析错误
            error_logger.log_parse_error(&file_path.to_string_lossy(), error)?;
            Ok(())
        },
    )?;

    // 刷新剩余批次
    if !local_buf.is_empty() {
        debug!("导出最后一批: {} 条记录", local_buf.len());
        exporter_manager.export_batch(&local_buf)?;
        local_buf.clear();
    }

    if total == 0 {
        warn!("没有解析到任何 SQL 日志记录");
        exporter_manager.finalize()?;
        error_logger.finalize()?;
        return Ok(());
    }

    info!("成功解析并分发 {} 条 SQL 日志记录", total);

    // 第六步：完成导出
    info!("完成导出...");
    exporter_manager.finalize()?;

    // 第七步：完成错误日志记录（生成 summary 指标文件）
    error_logger.finalize()?;
    info!("错误指标摘要文件: {}", error_logger.summary_path());

    // 展示统计信息
    exporter_manager.log_stats();

    info!("✓ SQL 日志导出任务完成！");
    info!("  - 解析记录数: {}", total);
    info!("  - 导出器数量: {}", exporter_manager.count());

    Ok(())
}

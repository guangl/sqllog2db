use tracing::{info, warn};

use crate::config::Config;
use crate::error::Result;
use crate::error_logger::ErrorLogger;
use crate::exporter::ExporterManager;
use crate::parser::SqllogParser;
use dm_database_parser_sqllog::Sqllog;

/// 运行日志导出任务（单线程、单导出器架构）
pub fn handle_run(cfg: &Config) -> Result<()> {
    info!("开始运行 SQL 日志导出任务");

    // 第一步：创建 SQL 日志解析器
    let parser = SqllogParser::new(cfg.sqllog.directory());
    info!("SQL 日志输入目录: {}", parser.path().display());

    // 第二步：创建导出器管理器（单个导出器）
    let mut exporter_manager = ExporterManager::from_config(cfg)?;
    info!("使用导出器: {}", exporter_manager.name());

    // 第三步：创建错误日志记录器
    let mut error_logger = ErrorLogger::new(cfg.error.file())?;

    // 第四步：初始化导出器
    info!("初始化导出器...");
    exporter_manager.initialize()?;

    // 第五步：解析 SQL 日志（流式）并导出
    info!("解析并导出 SQL 日志（流式）...");
    let batch_size = exporter_manager.batch_size();
    let mut total: usize = 0;
    let mut error_count: usize = 0;
    let mut local_buf: Vec<Sqllog> = Vec::new();
    let log_every = if batch_size > 0 {
        batch_size.max(1000)
    } else {
        1000
    };

    // 获取所有日志文件
    let log_files = parser.log_files()?;

    if log_files.is_empty() {
        warn!("没有找到任何日志文件");
        exporter_manager.finalize()?;
        error_logger.finalize()?;
        return Ok(());
    }

    info!("找到 {} 个日志文件", log_files.len());

    // 遍历每个日志文件
    for log_file in log_files {
        let file_path_str = log_file.to_string_lossy().to_string();
        info!("处理文件: {}", file_path_str);

        // 使用 dm-database-parser-sqllog 解析文件
        match dm_database_parser_sqllog::iter_records_from_file(&file_path_str) {
            Ok(iter) => {
                for result in iter {
                    match result {
                        Ok(record) => {
                            total += 1;

                            if batch_size > 0 {
                                // 批量模式：累积到本地缓冲
                                local_buf.push(record);
                                if local_buf.len() >= batch_size {
                                    exporter_manager.export_batch(&local_buf)?;
                                    local_buf.clear();
                                }
                            } else {
                                // 单条模式（batch_size=0）：累积所有记录，最后一次性导出
                                local_buf.push(record);
                            }

                            if total % log_every == 0 {
                                info!("已解析 {} 条记录...", total);
                            }
                        }
                        Err(e) => {
                            error_count += 1;
                            // 记录解析错误
                            if let Err(log_err) = error_logger.log_parse_error(&file_path_str, &e) {
                                warn!("记录解析错误失败: {}", log_err);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error_count += 1;
                warn!("无法打开文件 {}: {}", file_path_str, e);
                if let Err(log_err) = error_logger.log_parse_error(&file_path_str, &e) {
                    warn!("记录文件错误失败: {}", log_err);
                }
            }
        }
    }

    // 刷新剩余批次
    if !local_buf.is_empty() {
        info!("导出最后一批: {} 条记录", local_buf.len());
        exporter_manager.export_batch(&local_buf)?;
        local_buf.clear();
    }

    if total == 0 {
        warn!("没有解析到任何 SQL 日志记录");
        exporter_manager.finalize()?;
        error_logger.finalize()?;
        return Ok(());
    }

    info!("成功解析 {} 条 SQL 日志记录", total);
    if error_count > 0 {
        warn!("解析过程中遇到 {} 个错误", error_count);
    }

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
    info!("  - 解析错误数: {}", error_count);
    info!("  - 导出器: {}", exporter_manager.name());

    Ok(())
}

use crate::error::{Error, Result};
use crate::error_logger::ErrorLogger;
use crate::exporter::ExporterManager;
use crate::parser::SqllogParser;
use crate::{config::Config, error::ParserError};
use dm_database_parser_sqllog::LogParser;
use log::{info, warn};
use std::time::Instant;
use std::sync::{Arc, Mutex};
use std::io;

#[cfg(feature = "tui")]
use dm_database_sqllog2db::tui::TuiApp;

/// 处理单个日志文件（带 TUI 状态更新）
fn process_log_file_with_tui(
    file_index: usize,
    file_path: &str,
    exporter_manager: &mut ExporterManager,
    error_logger: &mut ErrorLogger,
    app_state: &Arc<Mutex<TuiApp>>,
) -> Result<()> {
    info!("Processing file: {file_path}");

    // 更新 TUI 显示当前文件
    {
        let mut app = app_state.lock().unwrap();
        app.set_file(
            file_index + 1,
            file_path
                .split(std::path::MAIN_SEPARATOR)
                .last()
                .unwrap_or(file_path)
                .to_string(),
        );
    }

    let parser = LogParser::from_path(file_path).map_err(|e| {
        Error::Parser(ParserError::InvalidPath {
            path: file_path.into(),
            reason: format!("{e}"),
        })
    })?;

    let mut batch = Vec::with_capacity(1000);
    for result in parser.iter() {
        match result {
            Ok(record) => {
                batch.push(record);
                if batch.len() >= 1000 {
                    exporter_manager.export_batch(&batch)?;
                    {
                        let mut app = app_state.lock().unwrap();
                        app.add_records(batch.len());
                    }
                    batch.clear();
                }
            }
            Err(e) => {
                // 如果有未处理的批次，先导出
                if !batch.is_empty() {
                    exporter_manager.export_batch(&batch)?;
                    {
                        let mut app = app_state.lock().unwrap();
                        app.add_records(batch.len());
                    }
                    batch.clear();
                }
                // 记录解析错误
                if let Err(log_err) = error_logger.log_parse_error(file_path, &e) {
                    warn!("Failed to record parse error: {log_err}");
                }
                {
                    let mut app = app_state.lock().unwrap();
                    app.add_errors(1);
                }
            }
        }
    }

    // 处理剩余的批次
    if !batch.is_empty() {
        exporter_manager.export_batch(&batch)?;
        {
            let mut app = app_state.lock().unwrap();
            app.add_records(batch.len());
        }
    }

    Ok(())
}

/// 运行日志导出任务（TUI 模式）
#[cfg(feature = "tui")]
pub async fn handle_run_tui(cfg: &Config) -> Result<()> {
    use dm_database_sqllog2db::tui::{TuiApp, run_tui};
    
    info!("Starting SQL log export task (TUI mode)");

    let parser = SqllogParser::new(cfg.sqllog.directory());
    info!("SQL log input directory: {}", parser.path().display());

    let log_files = parser.log_files()?;

    if log_files.is_empty() {
        warn!("No log files found");
        return Ok(());
    }

    info!("Found {} log file(s)", log_files.len());

    // 创建 TUI 应用状态
    let exporter_name = "TUI Export".to_string();
    let app_state = Arc::new(Mutex::new(TuiApp::new(log_files.len(), exporter_name)));
    {
        let mut app = app_state.lock().unwrap();
        app.start();
    }

    // 在后台运行导出任务
    let app_state_clone = app_state.clone();
    let cfg_clone = cfg.clone();
    let handle = tokio::task::spawn_blocking(move || {
        let total_start = Instant::now();
        let mut exporter_manager = match ExporterManager::from_config(&cfg_clone) {
            Ok(m) => m,
            Err(e) => {
                log::error!("Failed to create exporter: {e}");
                return Err(e);
            }
        };
        let mut error_logger = match ErrorLogger::new(cfg_clone.error.file()) {
            Ok(l) => l,
            Err(e) => {
                log::error!("Failed to create error logger: {e}");
                return Err(Error::from(e));
            }
        };

        if let Err(e) = exporter_manager.initialize() {
            log::error!("Failed to initialize exporter: {e}");
            return Err(e);
        }

        let parser = SqllogParser::new(cfg_clone.sqllog.directory());
        let log_files = match parser.log_files() {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to get log files: {e}");
                return Err(e);
            }
        };

        for (idx, log_file) in log_files.iter().enumerate() {
            let file_path_str = log_file.to_string_lossy().to_string();
            info!(
                "Processing file {}/{}: {}",
                idx + 1,
                log_files.len(),
                log_file.display()
            );

            if let Err(e) = process_log_file_with_tui(
                idx,
                &file_path_str,
                &mut exporter_manager,
                &mut error_logger,
                &app_state_clone,
            ) {
                log::error!("Error processing file {}: {e}", log_file.display());
            }
        }

        if let Err(e) = exporter_manager.finalize() {
            log::error!("Failed to finalize exporter: {e}");
            return Err(e);
        }

        if let Err(e) = error_logger.finalize() {
            log::error!("Failed to finalize error logger: {e}");
            return Err(Error::from(e));
        }

        let total_elapsed = total_start.elapsed().as_secs_f64();

        {
            let mut app = app_state_clone.lock().unwrap();
            app.finish();
        }

        info!(
            "✓ SQL log export task completed in {total_elapsed:.3}s!",
        );

        Ok(())
    });

    // 运行 TUI
    let tui_result = run_tui(app_state.clone()).await;

    // 等待后台任务完成
    match handle.await {
        Ok(Ok(())) => {
            info!("Export task completed successfully");
        }
        Ok(Err(e)) => {
            warn!("Export task failed: {e}");
            return Err(e);
        }
        Err(e) => {
            warn!("Export task panicked: {e}");
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::Other,
                format!("Export task failed: {e}"),
            )));
        }
    }

    // 处理 TUI 结果
    if let Err(e) = tui_result {
        warn!("TUI error: {e}");
    }

    Ok(())
}

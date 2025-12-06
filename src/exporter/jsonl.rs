use super::util::ensure_parent_dir;
use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{info, warn};
use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// JSONL 记录结构
#[derive(Debug, Serialize)]
struct JsonlRecord {
    ts: String,
    ep: u8,
    sess_id: String,
    thrd_id: String,
    username: String,
    trx_id: String,
    statement: String,
    appname: String,
    client_ip: String,
    sql: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    exec_time_ms: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    row_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exec_id: Option<i64>,
}

/// JSONL 导出器 - 将 SQL 日志导出为 JSON Lines 格式
pub struct JsonlExporter {
    path: PathBuf,
    overwrite: bool,
    append: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
}

impl JsonlExporter {
    /// 创建新的 JSONL 导出器
    pub fn new(path: impl AsRef<Path>, overwrite: bool) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            overwrite,
            append: false,
            writer: None,
            stats: ExportStats::new(),
        }
    }

    /// 从配置创建 JSONL 导出器
    pub fn from_config(config: &crate::config::JsonlExporter) -> Self {
        let mut exporter = Self::new(&config.file, config.overwrite);
        // 追加模式优先级高于 overwrite
        if config.append {
            exporter.overwrite = false;
            exporter.append = true;
        }
        exporter
    }

    /// 将 Sqllog 转换为 JsonlRecord
    fn sqllog_to_jsonl_record(sqllog: &Sqllog<'_>) -> JsonlRecord {
        let meta = sqllog.parse_meta();
        let ind = sqllog.parse_indicators();

        JsonlRecord {
            ts: sqllog.ts.to_string(),
            ep: meta.ep,
            sess_id: meta.sess_id.to_string(),
            thrd_id: meta.thrd_id.to_string(),
            username: meta.username.to_string(),
            trx_id: meta.trxid.to_string(),
            statement: meta.statement.to_string(),
            appname: meta.appname.to_string(),
            client_ip: meta.client_ip.to_string(),
            sql: sqllog.body().to_string(),
            exec_time_ms: ind.as_ref().map(|i| i.execute_time),
            row_count: ind.as_ref().map(|i| i.row_count),
            exec_id: ind.as_ref().map(|i| i.execute_id),
        }
    }
}

impl Exporter for JsonlExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing JSONL exporter: {}", self.path.display());

        ensure_parent_dir(&self.path).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to create directory: {}", e),
            })
        })?;

        let append_mode = self.append;

        let file = if append_mode {
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
        } else {
            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(self.overwrite)
                .open(&self.path)
        };

        let file = file.map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to open file: {}", e),
            })
        })?;

        self.writer = Some(BufWriter::new(file));

        info!("JSONL exporter initialized: {}", self.path.display());
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        // 检查是否已初始化
        if self.writer.is_none() {
            return Err(Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "JSONL exporter not initialized".to_string(),
            }));
        }

        // 转换为 JSONL 记录
        let record = Self::sqllog_to_jsonl_record(sqllog);

        // 序列化为 JSON
        let json_line = serde_json::to_string(&record).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to serialize to JSON: {}", e),
            })
        })?;

        // 写入 JSON 行
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "JSONL exporter not initialized".to_string(),
            })
        })?;

        writeln!(writer, "{}", json_line).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to write JSONL line: {}", e),
            })
        })?;

        self.stats.record_success();

        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog<'_>]) -> Result<()> {
        if sqllogs.is_empty() {
            return Ok(());
        }

        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "JSONL exporter not initialized".to_string(),
            })
        })?;

        // 内存优化：流式处理避免峰值
        // 分块处理（每 500 条），避免存储大量 String
        const CHUNK_SIZE: usize = 500;
        for chunk in sqllogs.chunks(CHUNK_SIZE) {
            let json_lines: Vec<String> = chunk
                .par_iter()
                .map(|sqllog| {
                    let record = Self::sqllog_to_jsonl_record(sqllog);
                    serde_json::to_string(&record).unwrap_or_default()
                })
                .collect();

            for json_line in json_lines {
                writeln!(writer, "{}", json_line).map_err(|e| {
                    Error::Export(ExportError::CsvExportFailed {
                        path: self.path.clone(),
                        reason: format!("Failed to write JSONL line: {}", e),
                    })
                })?;
                self.stats.record_success();
            }
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush().map_err(|e| {
                Error::Export(ExportError::CsvExportFailed {
                    path: self.path.clone(),
                    reason: format!("Failed to flush buffer: {}", e),
                })
            })?;

            info!(
                "JSONL export finished: {} (success: {}, failed: {})",
                self.path.display(),
                self.stats.exported,
                self.stats.failed
            );
        } else {
            warn!("JSONL exporter not initialized or already finished");
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "JSONL"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl Drop for JsonlExporter {
    fn drop(&mut self) {
        if self.writer.is_some()
            && let Err(e) = self.finalize()
        {
            warn!("JSONL exporter finalization on Drop failed: {}", e);
        }
    }
}

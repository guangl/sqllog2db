use super::util::ensure_parent_dir;
use super::{ExportStats, Exporter};
use crate::config;
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
// 移除模块内日志记录以降低开销
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// CSV 导出器 - 高性能批量写入版本
pub struct CsvExporter {
    path: PathBuf,
    overwrite: bool,
    append: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
    itoa_buf: itoa::Buffer, // itoa 复用缓冲区
    line_buf: Vec<u8>,      // 行缓冲区复用
}

impl CsvExporter {
    /// 创建新的 CSV 导出器
    pub fn new(path: impl AsRef<Path>, overwrite: bool) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            overwrite,
            append: false,
            writer: None,
            stats: ExportStats::new(),
            itoa_buf: itoa::Buffer::new(),      // itoa 缓冲区
            line_buf: Vec::with_capacity(1024), // 预分配 1KB
        }
    }

    /// 从配置创建 CSV 导出器
    pub fn from_config(config: &config::CsvExporter) -> Self {
        let mut exporter = Self::new(&config.file, config.overwrite);

        // 追加模式优先级高于 overwrite
        if config.append {
            exporter.overwrite = false;
            exporter.append = true;
        }
        exporter
    }
}

impl Exporter for CsvExporter {
    fn initialize(&mut self) -> Result<()> {
        // 初始化，无日志

        ensure_parent_dir(&self.path).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to create directory: {}", e),
            })
        })?;

        let append_mode = self.append;
        let file_exists = self.path.exists();

        let file = if append_mode {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
        } else {
            OpenOptions::new()
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

        // 16MB 缓冲区
        let mut writer = BufWriter::with_capacity(16 * 1024 * 1024, file);

        // 写入表头（如果需要）
        if !append_mode || !file_exists {
            writer.write_all(b"ts,ep,sess_id,thrd_id,username,trx_id,statement,appname,client_ip,sql,exec_time_ms,row_count,exec_id\n")
                .map_err(|e| {
                    Error::Export(ExportError::CsvExportFailed {
                        path: self.path.clone(),
                        reason: format!("Failed to write CSV header: {}", e),
                    })
                })?;
        }

        self.writer = Some(writer);

        // 初始化完成日志
        // 初始化完成，无日志
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        let meta = sqllog.parse_meta();
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "CSV exporter not initialized".to_string(),
            })
        })?;

        // 复用缓冲区
        self.line_buf.clear();
        let buf = &mut self.line_buf;

        // 时间戳 - 直接写入(不需要转义)
        buf.extend_from_slice(sqllog.ts.as_ref().as_bytes());
        buf.push(b',');

        // ep - 使用 itoa 快速整数转换
        buf.extend_from_slice(self.itoa_buf.format(meta.ep).as_bytes());
        buf.push(b',');

        // 字符串字段 - 直接写入(大部分不需要转义)
        buf.extend_from_slice(meta.sess_id.as_ref().as_bytes());
        buf.push(b',');
        buf.extend_from_slice(meta.thrd_id.as_ref().as_bytes());
        buf.push(b',');
        buf.extend_from_slice(meta.username.as_ref().as_bytes());
        buf.push(b',');
        buf.extend_from_slice(meta.trxid.as_ref().as_bytes());
        buf.push(b',');
        buf.extend_from_slice(meta.statement.as_ref().as_bytes());
        buf.push(b',');
        buf.extend_from_slice(meta.appname.as_ref().as_bytes());
        buf.push(b',');
        buf.extend_from_slice(meta.client_ip.as_ref().as_bytes());
        buf.push(b',');

        // SQL body - 仅为 SQL 字段进行转义（其余字段直接写入）
        // 优化：直接遍历字节，避免 UTF-8 解码开销
        buf.push(b'"');
        for &byte in sqllog.body().as_ref().as_bytes() {
            if byte == b'"' {
                buf.push(b'"');
                buf.push(b'"');
            } else {
                buf.push(byte);
            }
        }
        buf.push(b'"');
        buf.push(b',');

        // 性能指标 - 使用 itoa
        if let Some(indicators) = sqllog.parse_indicators() {
            buf.extend_from_slice(
                self.itoa_buf
                    .format(indicators.execute_time as i64)
                    .as_bytes(),
            );
            buf.push(b',');
            buf.extend_from_slice(self.itoa_buf.format(indicators.row_count as i64).as_bytes());
            buf.push(b',');
            buf.extend_from_slice(self.itoa_buf.format(indicators.execute_id).as_bytes());
            buf.push(b'\n');
        } else {
            buf.extend_from_slice(b",,\n");
        }

        // 直接写入
        writer.write_all(buf).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to write CSV line: {}", e),
            })
        })?;

        // 仅记录成功计数，避免过多统计开销
        self.stats.record_success();
        Ok(())
    }

    /// 刷新缓冲区并关闭
    fn finalize(&mut self) -> Result<()> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush().map_err(|e| {
                Error::Export(ExportError::CsvExportFailed {
                    path: self.path.clone(),
                    reason: format!("Failed to flush buffer: {}", e),
                })
            })?;
            // 完成，无日志
        } else {
            // 未初始化或已完成
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "CSV"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl CsvExporter {}

impl Drop for CsvExporter {
    fn drop(&mut self) {
        if self.writer.is_some() {
            let _ = self.finalize();
        }
    }
}

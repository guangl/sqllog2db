use super::util::ensure_parent_dir;
use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

// 使用 once_cell 缓存 CSV 头部，避免每次重新构建
static CSV_HEADER: Lazy<&str> = Lazy::new(
    || "ts,ep,sess_id,thrd_id,username,trx_id,statement,appname,client_ip,sql,exec_time_ms,row_count,exec_id\n",
);

/// 检查字段是否需要引号包围
#[inline]
fn needs_quoting(field: &str) -> bool {
    field.contains(',') || field.contains('"') || field.contains('\n')
}

/// CSV 导出器 - 将 SQL 日志导出为 CSV 格式
pub struct CsvExporter {
    path: PathBuf,
    overwrite: bool,
    append: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
    header_written: bool,
    line_buf: String, // 重用的行缓冲区
}

impl CsvExporter {
    /// 创建新的 CSV 导出器
    pub fn new(path: impl AsRef<Path>, overwrite: bool) -> Self {
        Self::with_batch_size(path, overwrite, 0)
    }

    /// 创建新的 CSV 导出器（指定批量大小）
    pub fn with_batch_size(path: impl AsRef<Path>, overwrite: bool, batch_size: usize) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            overwrite,
            append: false,
            writer: None,
            stats: ExportStats::new(),
            header_written: false,
            line_buf: String::with_capacity(batch_size),
        }
    }

    /// 从配置创建 CSV 导出器，支持自定义批量大小
    pub fn from_config(config: &crate::config::CsvExporter, batch_size: usize) -> Self {
        let mut exporter = if batch_size > 0 {
            Self::with_batch_size(&config.file, config.overwrite, batch_size)
        } else {
            Self::new(&config.file, config.overwrite)
        };
        // 追加模式优先级高于 overwrite
        if config.append {
            exporter.overwrite = false;
            exporter.append = true;
        }
        exporter
    }

    /// 写入 CSV 头部
    fn write_header(&mut self) -> Result<()> {
        if self.header_written {
            return Ok(());
        }

        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "CSV exporter not initialized".to_string(),
            })
        })?;

        // 使用预编译的 CSV 头部
        writer.write_all(CSV_HEADER.as_bytes()).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to write CSV header: {}", e),
            })
        })?;

        self.header_written = true;
        debug!("CSV header written");
        Ok(())
    }

    /// 写入 CSV 字段到缓冲区（避免分配）
    fn write_csv_field(buf: &mut String, field: &str) {
        if needs_quoting(field) {
            buf.push('"');
            for ch in field.chars() {
                if ch == '"' {
                    buf.push('"');
                    buf.push('"');
                } else {
                    buf.push(ch);
                }
            }
            buf.push('"');
        } else {
            buf.push_str(field);
        }
    }

    /// 将 Sqllog 转换为 CSV 行（优化版本，使用预分配缓冲区）
    fn sqllog_to_csv_line_into(sqllog: &Sqllog, buf: &mut String) {
        buf.clear();
        buf.reserve(256); // 预分配合理大小

        Self::write_csv_field(buf, &sqllog.ts);
        buf.push(',');

        use std::fmt::Write;
        let _ = write!(buf, "{},", sqllog.meta.ep);

        Self::write_csv_field(buf, &sqllog.meta.sess_id);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.thrd_id);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.username);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.trxid);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.statement);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.appname);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.client_ip);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.body);
        buf.push(',');

        // 性能指标
        if let Some(indicators) = &sqllog.indicators {
            let _ = write!(
                buf,
                "{},{},{}",
                indicators.execute_time, indicators.row_count, indicators.execute_id
            );
        } else {
            buf.push_str(",,");
        }

        buf.push('\n');
    }
}

impl Exporter for CsvExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing CSV exporter: {}", self.path.display());

        ensure_parent_dir(&self.path).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to create directory: {}", e),
            })
        })?;

        // 判断 append 模式（基于实例字段，不再读取全局默认配置）
        let append_mode = self.append;
        let file_exists = self.path.exists();

        let file = if append_mode {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
        } else {
            std::fs::OpenOptions::new()
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

        // 追加模式且文件已存在，不写表头
        if append_mode && file_exists {
            self.header_written = true;
        } else {
            self.write_header()?;
        }

        info!("CSV exporter initialized: {}", self.path.display());
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog) -> Result<()> {
        // 检查是否已初始化
        if self.writer.is_none() {
            return Err(Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "CSV exporter not initialized".to_string(),
            }));
        }

        // 使用重用缓冲区生成 CSV 行
        Self::sqllog_to_csv_line_into(sqllog, &mut self.line_buf);

        // 直接写入，避免额外的字符串克隆和缓冲
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "CSV exporter not initialized".to_string(),
            })
        })?;

        writer.write_all(self.line_buf.as_bytes()).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to write CSV row: {}", e),
            })
        })?;

        // 无论哪种模式，都记录成功（数据已被接受）
        self.stats.record_success();

        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog]) -> Result<()> {
        debug!("Exported {} records to CSV in batch", sqllogs.len());

        for sqllog in sqllogs {
            self.export(sqllog)?;
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
                "CSV export finished: {} (success: {}, failed: {})",
                self.path.display(),
                self.stats.exported,
                self.stats.failed
            );
        } else {
            warn!("CSV exporter not initialized or already finished");
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

impl Drop for CsvExporter {
    fn drop(&mut self) {
        if self.writer.is_some() {
            if let Err(e) = self.finalize() {
                warn!("CSV exporter finalization on Drop failed: {}", e);
            }
        }
    }
}

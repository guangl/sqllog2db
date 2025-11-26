use super::util::ensure_parent_dir;
use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{debug, info, warn};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

/// Parquet 导出器 - 将 SQL 日志导出为 Parquet 格式
pub struct ParquetExporter {
    path: PathBuf,
    overwrite: bool,
    append: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
    header_written: bool,
    line_buf: String, // 重用的行缓冲区
}

impl ParquetExporter {
    /// 从配置创建 Parquet 导出器
    pub fn from_config(config: &crate::config::ParquetExporter) -> Self {
        Self {
            path: config.file.clone().into(),
            overwrite: config.overwrite,
            append: config.append,
            writer: None,
            stats: ExportStats::new(),
            header_written: false,
            line_buf: String::with_capacity(0),
        }
    }
}

impl Exporter for ParquetExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing Parquet exporter: {}", self.path.display());

        ensure_parent_dir(&self.path).map_err(|e| {
            Error::Export(ExportError::ParquetExportFailed {
                path: self.path.clone(),
                reason: format!("Failed to create directory: {}", e),
            })
        })?;

        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog) -> Result<()> {
        // 检查是否已初始化
        if self.writer.is_none() {
            return Err(Error::Export(ExportError::ParquetExportFailed {
                path: self.path.clone(),
                reason: "Parquet exporter not initialized".to_string(),
            }));
        }

        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog]) -> Result<()> {
        debug!("Exported {} records to Parquet in batch", sqllogs.len());

        for sqllog in sqllogs {
            self.export(sqllog)?;
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "Parquet"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl Drop for ParquetExporter {
    fn drop(&mut self) {
        if self.writer.is_some() {
            if let Err(e) = self.finalize() {
                warn!("Parquet exporter finalization on Drop failed: {}", e);
            }
        }
    }
}

use crate::error::Result;
use crate::exporter::ExportStats;
use dm_database_parser_sqllog::Sqllog;
use log::{debug, info};
use std::path::Path;

/// Parquet 导出器（无压缩功能）
pub struct ParquetExporter {
    pub file: String,
    pub overwrite: bool,
    pub row_group_size: usize,
    pub use_dictionary: bool,
    pub stats: ExportStats,
    pub pending_records: Vec<Sqllog>,
}

impl ParquetExporter {
    pub fn new(file: String, overwrite: bool, row_group_size: usize, use_dictionary: bool) -> Self {
        Self {
            file,
            overwrite,
            row_group_size,
            use_dictionary,
            stats: ExportStats::new(),
            pending_records: Vec::with_capacity(row_group_size),
        }
    }

    /// 初始化 Parquet 文件（如有需要可创建目录/文件）
    pub fn initialize(&mut self) -> Result<()> {
        let path = Path::new(&self.file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if self.overwrite && path.exists() {
            std::fs::remove_file(path)?;
        }
        info!("ParquetExporter initialized: {}", self.file);
        Ok(())
    }

    /// 导出单条记录
    pub fn export(&mut self, sqllog: &Sqllog) -> Result<()> {
        self.pending_records.push(sqllog.clone());
        if self.pending_records.len() >= self.row_group_size {
            self.flush()?;
        }
        Ok(())
    }

    /// 批量导出
    pub fn export_batch(&mut self, sqllogs: &[&Sqllog]) -> Result<()> {
        for sqllog in sqllogs {
            self.pending_records.push((*sqllog).clone());
            if self.pending_records.len() >= self.row_group_size {
                self.flush()?;
            }
        }
        Ok(())
    }

    /// 刷新写入 Parquet 文件（实际写入逻辑需集成 parquet crate）
    pub fn flush(&mut self) -> Result<()> {
        // TODO: 使用 parquet crate 写入 self.pending_records 到 self.file
        debug!("Flush {} records to Parquet", self.pending_records.len());
        self.pending_records.clear();
        Ok(())
    }

    /// 完成导出，写入剩余数据
    pub fn finalize(&mut self) -> Result<()> {
        self.flush()?;
        info!("Parquet export finished: {} records", self.stats.exported);
        Ok(())
    }

    pub fn name(&self) -> &str {
        "Parquet"
    }

    pub fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

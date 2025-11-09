/// Exporter 模块 - 负责将解析后的 SQL 日志导出到各种目标
///
/// 支持的导出目标:
/// - CSV 文件
/// - JSONL (JSON Lines) 文件
/// - 数据库 (DuckDB, SQLite, PostgreSQL, Oracle, DM)
use crate::config::Config;
use crate::error::Result;
use dm_database_parser_sqllog::Sqllog;
use tracing::{debug, info};

#[cfg(feature = "csv")]
mod csv;
mod database;
#[cfg(feature = "jsonl")]
mod jsonl;
mod util;

#[cfg(feature = "csv")]
pub use csv::CsvExporter;
pub use database::DatabaseExporter;
#[cfg(feature = "jsonl")]
pub use jsonl::JsonlExporter;

/// Exporter 基础 trait - 所有导出器必须实现此接口
/// 导出器 trait
pub trait Exporter: Send {
    /// 初始化导出器 (例如:创建文件、连接数据库、创建表等)
    fn initialize(&mut self) -> Result<()>;

    /// 导出单条 SQL 日志记录
    fn export(&mut self, sqllog: &Sqllog) -> Result<()>;

    /// 批量导出多条日志记录 (默认实现:逐条调用 export)
    fn export_batch(&mut self, sqllogs: &[Sqllog]) -> Result<()> {
        for sqllog in sqllogs {
            self.export(sqllog)?;
        }
        Ok(())
    }

    /// 完成导出 (例如:刷新缓冲区、提交事务、关闭文件等)
    fn finalize(&mut self) -> Result<()>;

    /// 获取导出器名称 (用于日志记录)
    fn name(&self) -> &str;

    /// 获取导出统计信息的快照
    /// 默认返回 None；具体导出器可覆盖此方法以提供统计信息
    fn stats_snapshot(&self) -> Option<ExportStats> {
        None
    }
}

/// 导出统计信息
#[derive(Debug, Default, Clone)]
pub struct ExportStats {
    /// 成功导出的记录数
    pub exported: usize,
    /// 跳过的记录数
    pub skipped: usize,
    /// 失败的记录数
    pub failed: usize,
    /// 刷新/批量写入操作次数（数据库类导出器）
    pub flush_operations: usize,
    /// 最近一次刷新写入的记录数
    pub last_flush_size: usize,
}

impl ExportStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_success(&mut self) {
        self.exported += 1;
    }

    pub fn record_skip(&mut self) {
        self.skipped += 1;
    }

    pub fn record_failure(&mut self) {
        self.failed += 1;
    }

    pub fn total(&self) -> usize {
        self.exported + self.skipped + self.failed
    }

    /// 记录一次刷新操作
    pub fn record_flush(&mut self, count: usize) {
        self.flush_operations += 1;
        self.last_flush_size = count;
    }
}

/// 导出器管理器 - 统一管理所有导出器
pub struct ExporterManager {
    exporters: Vec<Box<dyn Exporter>>,
    batch_size: usize,
}

impl ExporterManager {
    /// 从配置创建导出器管理器
    pub fn from_config(config: &Config) -> Result<Self> {
        let mut exporters: Vec<Box<dyn Exporter>> = Vec::new();
        let batch_size = config.sqllog.batch_size();

        info!("初始化导出器管理器...");

        // 创建 CSV 导出器（需启用 feature="csv"）
        #[cfg(feature = "csv")]
        {
            for csv_config in config.exporter.csvs() {
                let csv_exporter = CsvExporter::from_config(csv_config, batch_size);
                debug!("添加 CSV 导出器: {}", csv_config.path);
                exporters.push(Box::new(csv_exporter));
            }
        }
        #[cfg(not(feature = "csv"))]
        {
            if !config.exporter.csvs().is_empty() {
                info!(
                    "CSV 导出器特性未启用, 跳过 {} 个 CSV 导出配置",
                    config.exporter.csvs().len()
                );
            }
        }

        // 创建 JSONL 导出器（需启用 feature="jsonl"）
        #[cfg(feature = "jsonl")]
        {
            for jsonl_config in config.exporter.jsonls() {
                let jsonl_exporter = JsonlExporter::from_config(jsonl_config, batch_size);
                debug!("添加 JSONL 导出器: {}", jsonl_config.path);
                exporters.push(Box::new(jsonl_exporter));
            }
        }
        #[cfg(not(feature = "jsonl"))]
        {
            if !config.exporter.jsonls().is_empty() {
                info!(
                    "JSONL 导出器特性未启用, 跳过 {} 个 JSONL 导出配置",
                    config.exporter.jsonls().len()
                );
            }
        }

        // 创建数据库导出器
        for db_config in config.exporter.databases() {
            let db_exporter = DatabaseExporter::from_config(db_config);
            debug!(
                "添加数据库导出器: {} ({})",
                db_config.table_name,
                db_config.database_type.as_str()
            );
            exporters.push(Box::new(db_exporter));
        }

        info!("导出器管理器初始化完成，共 {} 个导出器", exporters.len());

        Ok(Self {
            exporters,
            batch_size,
        })
    }

    /// 初始化所有导出器
    pub fn initialize(&mut self) -> Result<()> {
        info!("初始化所有导出器...");
        for exporter in &mut self.exporters {
            exporter.initialize()?;
        }
        info!("所有导出器初始化完成");
        Ok(())
    }

    /// 导出单条日志记录到所有导出器
    pub fn export(&mut self, sqllog: &Sqllog) -> Result<()> {
        for exporter in &mut self.exporters {
            exporter.export(sqllog)?;
        }
        Ok(())
    }

    /// 批量导出日志记录到所有导出器
    pub fn export_batch(&mut self, sqllogs: &[Sqllog]) -> Result<()> {
        use std::time::Instant;
        let start = Instant::now();
        let count = sqllogs.len();
        if count == 0 {
            return Ok(());
        }

        let names: Vec<&str> = self.exporters.iter().map(|e| e.name()).collect();
        info!("批量导出: {} 条记录 -> [{}]", count, names.join(", "));

        for exporter in &mut self.exporters {
            exporter.export_batch(sqllogs)?;
        }

        let elapsed = start.elapsed();
        info!(
            "批量导出完成: {} 条记录, 用时 {:?} (平均 {:?}/记录)",
            count,
            elapsed,
            elapsed / (count as u32)
        );
        Ok(())
    }

    /// 完成所有导出器
    pub fn finalize(&mut self) -> Result<()> {
        info!("完成所有导出器...");
        for exporter in &mut self.exporters {
            exporter.finalize()?;
        }
        info!("所有导出器已完成");
        Ok(())
    }

    /// 获取导出器数量
    pub fn count(&self) -> usize {
        self.exporters.len()
    }

    /// 获取批量大小配置
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// 收集所有导出器的统计信息
    pub fn stats(&self) -> Vec<(String, ExportStats)> {
        let mut out = Vec::with_capacity(self.exporters.len());
        for exp in &self.exporters {
            if let Some(s) = exp.stats_snapshot() {
                out.push((exp.name().to_string(), s));
            }
        }
        out
    }

    /// 记录各导出器的统计信息到日志
    pub fn log_stats(&self) {
        let stats = self.stats();
        if stats.is_empty() {
            info!("无可用的导出统计信息");
            return;
        }
        info!("导出统计信息:");
        for (name, s) in stats {
            info!(
                "  - {:<8} => 成功: {}, 失败: {}, 跳过: {} (合计: {}){}",
                name,
                s.exported,
                s.failed,
                s.skipped,
                s.total(),
                if s.flush_operations > 0 {
                    format!(
                        " | 刷新:{} 次 (最近 {} 条)",
                        s.flush_operations, s.last_flush_size
                    )
                } else {
                    String::new()
                }
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_stats_new() {
        let stats = ExportStats::new();
        assert_eq!(stats.exported, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn test_export_stats_record() {
        let mut stats = ExportStats::new();

        stats.record_success();
        stats.record_success();
        assert_eq!(stats.exported, 2);

        stats.record_skip();
        assert_eq!(stats.skipped, 1);

        stats.record_failure();
        assert_eq!(stats.failed, 1);

        assert_eq!(stats.total(), 4);
    }
}

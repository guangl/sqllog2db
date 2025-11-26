/// Exporter 模块 - 负责将解析后的 SQL 日志导出到各种目标
///
/// 支持的导出目标:
/// - CSV 文件
/// - SQLite 数据库
use crate::config::Config;
use crate::error::Result;
use dm_database_parser_sqllog::Sqllog;
use log::info;

#[cfg(feature = "csv")]
mod csv;
#[cfg(feature = "parquet")]
mod parquet;
mod util;

#[cfg(feature = "csv")]
pub use csv::CsvExporter;
#[cfg(feature = "parquet")]
pub use parquet::ParquetExporter;

/// Exporter 基础 trait - 所有导出器必须实现此接口
/// 导出器 trait
pub trait Exporter {
    /// 初始化导出器 (例如:创建文件、连接数据库、创建表等)
    fn initialize(&mut self) -> Result<()>;

    /// 导出单条 SQL 日志记录
    fn export(&mut self, sqllog: &Sqllog) -> Result<()>;

    /// 批量导出多条日志记录 (默认实现:逐条调用 export)
    fn export_batch(&mut self, sqllogs: &[&Sqllog]) -> Result<()> {
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
}

impl ExportStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_success(&mut self) {
        self.exported += 1;
    }

    pub fn total(&self) -> usize {
        self.exported + self.skipped + self.failed
    }
}

/// 导出器管理器 - 管理单个导出器
pub struct ExporterManager {
    exporter: Box<dyn Exporter>,
}

impl ExporterManager {
    /// 从配置创建导出器管理器
    pub fn from_config(config: &Config) -> Result<Self> {
        info!("Initializing exporter manager...");

        // 优先级：CSV > Parquet > SQLite

        // 1. 尝试创建 CSV 导出器
        #[cfg(feature = "csv")]
        if let Some(csv_config) = config.exporter.csv() {
            let csv_exporter = CsvExporter::from_config(csv_config);
            info!("Using CSV exporter: {}", csv_config.file);
            return Ok(Self {
                exporter: Box::new(csv_exporter),
            });
        }

        // 2. 尝试创建 Parquet 导出器
        #[cfg(feature = "parquet")]
        if let Some(parquet_config) = config.exporter.parquet() {
            let parquet_exporter = ParquetExporter::from_config(parquet_config);
            info!("Using Parquet exporter: {}", parquet_config.file);
            return Ok(Self {
                exporter: Box::new(parquet_exporter),
            });
        }

        // 3. 其它导出器（如 SQLite）可继续补充

        Err(crate::error::Error::Config(
            crate::error::ConfigError::NoExporters,
        ))
    }
    /// 初始化导出器
    pub fn initialize(&mut self) -> Result<()> {
        info!("Initializing exporters...");
        self.exporter.initialize()?;
        info!("Exporters initialized");
        Ok(())
    }

    /// 批量导出日志记录
    pub fn export_batch(&mut self, sqllogs: &[Sqllog]) -> Result<()> {
        if sqllogs.is_empty() {
            return Ok(());
        }

        // 转换为引用的切片
        let refs: Vec<&Sqllog> = sqllogs.iter().collect();
        self.exporter.export_batch(&refs)
    }

    /// 完成导出器
    pub fn finalize(&mut self) -> Result<()> {
        info!("Finalizing exporters...");
        self.exporter.finalize()?;
        info!("Exporters finished");
        Ok(())
    }

    /// 获取导出器名称
    pub fn name(&self) -> &str {
        self.exporter.name()
    }

    /// 获取导出统计信息
    pub fn stats(&self) -> Option<ExportStats> {
        self.exporter.stats_snapshot()
    }

    /// 记录导出器的统计信息到日志
    pub fn log_stats(&self) {
        if let Some(s) = self.stats() {
            info!(
                "Export stats: {} => success: {}, failed: {}, skipped: {} (total: {})",
                self.name(),
                s.exported,
                s.failed,
                s.skipped,
                s.total(),
            );
        } else {
            info!("No export statistics available");
        }
    }
}

/// Exporter 模块 - 负责将解析后的 SQL 日志导出到各种目标
///
/// 支持的导出目标:
/// - CSV 文件
/// - SQLite 数据库
use crate::config::Config;
use crate::error::Result;
use dm_database_parser_sqllog::Sqllog;
use tracing::info;

#[cfg(feature = "csv")]
mod csv;
#[cfg(feature = "sqlite")]
pub mod database;
mod util;

#[cfg(feature = "csv")]
pub use csv::CsvExporter;
#[cfg(feature = "sqlite")]
pub use database::SQLiteExporter;

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

    pub fn total(&self) -> usize {
        self.exported + self.skipped + self.failed
    }
}

/// 导出器管理器 - 管理单个导出器
pub struct ExporterManager {
    exporter: Box<dyn Exporter>,
    batch_size: usize,
}

impl ExporterManager {
    /// 从配置创建导出器管理器
    pub fn from_config(config: &Config) -> Result<Self> {
        let batch_size = config.sqllog.batch_size();

        info!("初始化导出器管理器...");

        // 优先级：CSV > SQLite

        // 1. 尝试创建 CSV 导出器
        #[cfg(feature = "csv")]
        if let Some(csv_config) = config.exporter.csv() {
            let csv_exporter = CsvExporter::from_config(csv_config, batch_size);
            info!("使用 CSV 导出器: {}", csv_config.file);
            return Ok(Self {
                exporter: Box::new(csv_exporter),
                batch_size,
            });
        }

        // 2. 尝试创建 SQLite 导出器
        #[cfg(feature = "sqlite")]
        if let Some(sqlite_config) = config.exporter.sqlite() {
            let exporter = SQLiteExporter::with_batch_size(
                sqlite_config.file.clone(),
                sqlite_config.table_name.clone(),
                sqlite_config.overwrite,
                sqlite_config.append,
                batch_size,
            );
            info!("使用 SQLite 导出器: {}", sqlite_config.file);
            return Ok(Self {
                exporter: Box::new(exporter),
                batch_size,
            });
        }

        Err(crate::error::Error::Config(
            crate::error::ConfigError::NoExporters,
        ))
    }
    /// 初始化导出器
    pub fn initialize(&mut self) -> Result<()> {
        info!("初始化导出器...");
        self.exporter.initialize()?;
        info!("导出器初始化完成");
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
        info!("完成导出器...");
        self.exporter.finalize()?;
        info!("导出器已完成");
        Ok(())
    }

    /// 获取批量大小配置
    pub fn batch_size(&self) -> usize {
        self.batch_size
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
                "导出统计信息: {} => 成功: {}, 失败: {}, 跳过: {} (合计: {}){}",
                self.name(),
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
        } else {
            info!("无可用的导出统计信息");
        }
    }
}

/// Exporter 模块 - 负责将解析后的 SQL 日志导出到各种目标
///
/// 支持的导出目标:
/// - CSV 文件
/// - `SQLite` 数据库
use crate::config::Config;
use crate::error::{ConfigError, Error, Result};
use dm_database_parser_sqllog::Sqllog;
use log::info;

#[cfg(feature = "csv")]
pub mod csv;
#[cfg(feature = "dm")]
pub mod dm;
#[cfg(feature = "duckdb")]
pub mod duckdb;
#[cfg(feature = "jsonl")]
pub mod jsonl;
#[cfg(feature = "parquet")]
pub mod parquet;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;
mod util;

#[cfg(feature = "csv")]
pub use csv::CsvExporter;
#[cfg(feature = "dm")]
pub use dm::DmExporter;
#[cfg(feature = "duckdb")]
pub use duckdb::DuckdbExporter;
#[cfg(feature = "jsonl")]
pub use jsonl::JsonlExporter;
#[cfg(feature = "parquet")]
pub use parquet::ParquetExporter;
#[cfg(feature = "postgres")]
pub use postgres::PostgresExporter;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteExporter;

/// Exporter 基础 trait - 所有导出器必须实现此接口
/// 导出器 trait
pub trait Exporter {
    /// 初始化导出器 (例如:创建文件、连接数据库、创建表等)
    fn initialize(&mut self) -> Result<()>;

    /// 导出单条 SQL 日志记录
    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()>;

    /// 批量导出多条日志记录 (默认实现:逐条调用 export)
    fn export_batch(&mut self, sqllogs: &[&Sqllog<'_>]) -> Result<()> {
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
    #[must_use] 
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_success(&mut self) {
        self.exported += 1;
    }

    #[must_use] 
    pub fn total(&self) -> usize {
        self.exported + self.skipped + self.failed
    }
}

/// 导出器管理器 - 管理单个导出器
pub struct ExporterManager {
    exporter: Box<dyn Exporter>,
}

impl std::fmt::Debug for ExporterManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExporterManager")
            .field("exporter_name", &self.exporter.name())
            .finish()
    }
}

impl ExporterManager {
    /// 从配置创建导出器管理器
    pub fn from_config(config: &Config) -> Result<Self> {
        info!("Initializing exporter manager...");

        // 优先级：CSV > Parquet > JSONL > SQLite > DM

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

        // 3. 尝试创建 JSONL 导出器
        #[cfg(feature = "jsonl")]
        if let Some(jsonl_config) = config.exporter.jsonl() {
            let jsonl_exporter = JsonlExporter::from_config(jsonl_config);
            info!("Using JSONL exporter: {}", jsonl_config.file);
            return Ok(Self {
                exporter: Box::new(jsonl_exporter),
            });
        }

        // 4. 尝试创建 SQLite 导出器
        #[cfg(feature = "sqlite")]
        if let Some(sqlite_config) = config.exporter.sqlite() {
            let sqlite_exporter = SqliteExporter::from_config(sqlite_config);
            info!("Using SQLite exporter: {}", sqlite_config.database_url);
            return Ok(Self {
                exporter: Box::new(sqlite_exporter),
            });
        }

        // 5. 尝试创建 DuckDB 导出器
        #[cfg(feature = "duckdb")]
        if let Some(duckdb_config) = config.exporter.duckdb() {
            let duckdb_exporter = DuckdbExporter::from_config(duckdb_config);
            info!("Using DuckDB exporter: {}", duckdb_config.database_url);
            return Ok(Self {
                exporter: Box::new(duckdb_exporter),
            });
        }

        // 6. 尝试创建 PostgreSQL 导出器
        #[cfg(feature = "postgres")]
        if let Some(postgres_config) = config.exporter.postgres() {
            let postgres_exporter = PostgresExporter::from_config(postgres_config);
            info!("Using PostgreSQL exporter");
            return Ok(Self {
                exporter: Box::new(postgres_exporter),
            });
        }

        // 7. 尝试创建 DM 导出器
        #[cfg(feature = "dm")]
        if let Some(dm_config) = config.exporter.dm() {
            let dm_exporter = DmExporter::from_config(dm_config);
            info!("Using DM exporter: {}", dm_config.userid);
            return Ok(Self {
                exporter: Box::new(dm_exporter),
            });
        }

        Err(Error::Config(ConfigError::NoExporters))
    }
    /// 初始化导出器
    pub fn initialize(&mut self) -> Result<()> {
        info!("Initializing exporters...");
        self.exporter.initialize()?;
        info!("Exporters initialized");
        Ok(())
    }

    /// 批量导出日志记录
    pub fn export_batch(&mut self, sqllogs: &[Sqllog<'_>]) -> Result<()> {
        if sqllogs.is_empty() {
            return Ok(());
        }

        // 转换为引用的切片
        let refs: Vec<&Sqllog<'_>> = sqllogs.iter().collect();
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
    #[must_use] 
    pub fn name(&self) -> &str {
        self.exporter.name()
    }

    /// 获取导出统计信息
    #[must_use] 
    pub fn stats(&self) -> Option<ExportStats> {
        self.exporter.stats_snapshot()
    }

    /// 记录导出器的统计信息到日志
    pub fn log_stats(&self) {
        if let Some(s) = self.stats() {
            info!(
                "Export stats: {} => success: {}, failed: {}, skipped: {} (total: {}){}",
                self.name(),
                s.exported,
                s.failed,
                s.skipped,
                s.total(),
                if s.flush_operations > 0 {
                    format!(
                        " | flushed:{} times (recent {} entries)",
                        s.flush_operations, s.last_flush_size
                    )
                } else {
                    String::new()
                }
            );
        } else {
            info!("No export statistics available");
        }
    }
}

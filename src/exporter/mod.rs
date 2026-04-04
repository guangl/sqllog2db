use crate::config::Config;
use crate::error::{ConfigError, Error, Result};
use dm_database_parser_sqllog::Sqllog;
use log::info;

#[cfg(feature = "csv")]
pub mod csv;
#[cfg(feature = "jsonl")]
pub mod jsonl;
#[cfg(feature = "sqlite")]
pub mod sqlite;
mod util;

#[cfg(feature = "csv")]
pub use csv::CsvExporter;
#[cfg(feature = "jsonl")]
pub use jsonl::JsonlExporter;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteExporter;

/// 所有导出器必须实现的接口
pub trait Exporter {
    fn initialize(&mut self) -> Result<()>;
    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()>;

    /// 批量导出（默认逐条调用 export）
    fn export_batch(&mut self, sqllogs: &[Sqllog<'_>]) -> Result<()> {
        for sqllog in sqllogs {
            self.export(sqllog)?;
        }
        Ok(())
    }

    /// 批量导出，同时传入每条记录对应的 `normalized_sql`（仅 `replace_parameters` 特性使用）。
    /// 默认实现忽略 normalized 参数，直接调用 `export_batch`。
    #[cfg(feature = "replace_parameters")]
    fn export_batch_with_normalized(
        &mut self,
        sqllogs: &[Sqllog<'_>],
        normalized: &[Option<String>],
    ) -> Result<()> {
        let _ = normalized;
        self.export_batch(sqllogs)
    }

    fn finalize(&mut self) -> Result<()>;
    fn name(&self) -> &str;

    fn stats_snapshot(&self) -> Option<ExportStats> {
        None
    }
}

/// 导出统计
#[derive(Debug, Default, Clone)]
pub struct ExportStats {
    pub exported: usize,
    pub skipped: usize,
    pub failed: usize,
    pub flush_operations: usize,
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

    pub fn record_success_batch(&mut self, count: usize) {
        self.exported += count;
    }

    #[must_use]
    pub fn total(&self) -> usize {
        self.exported + self.skipped + self.failed
    }
}

/// 空运行导出器：只计数，不写任何文件（用于 --dry-run 模式）
#[derive(Debug, Default)]
pub struct DryRunExporter {
    stats: ExportStats,
}

impl Exporter for DryRunExporter {
    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn export(&mut self, _sqllog: &Sqllog<'_>) -> Result<()> {
        self.stats.exported += 1;
        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[Sqllog<'_>]) -> Result<()> {
        self.stats.exported += sqllogs.len();
        self.stats.flush_operations += 1;
        self.stats.last_flush_size = sqllogs.len();
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "dry-run"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

/// 导出器管理器
pub struct ExporterManager {
    exporter: Box<dyn Exporter>,
}

impl std::fmt::Debug for ExporterManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExporterManager")
            .field("exporter", &self.exporter.name())
            .finish()
    }
}

impl ExporterManager {
    /// 创建空运行导出器，只统计记录数不写文件
    #[must_use]
    pub fn dry_run() -> Self {
        info!("Dry-run mode: no output will be written");
        Self {
            exporter: Box::new(DryRunExporter::default()),
        }
    }

    pub fn from_config(config: &Config) -> Result<Self> {
        info!("Initializing exporter manager...");

        #[cfg(feature = "replace_parameters")]
        let normalize = config
            .features
            .replace_parameters
            .as_ref()
            .is_none_or(|r| r.enable);

        #[cfg(feature = "csv")]
        if let Some(cfg) = &config.exporter.csv {
            info!("Using CSV exporter: {}", cfg.file);
            #[cfg_attr(not(feature = "replace_parameters"), allow(unused_mut))]
            let mut exporter = CsvExporter::from_config(cfg);
            #[cfg(feature = "replace_parameters")]
            {
                exporter.normalize = normalize;
            }
            return Ok(Self {
                exporter: Box::new(exporter),
            });
        }

        #[cfg(feature = "jsonl")]
        if let Some(cfg) = &config.exporter.jsonl {
            info!("Using JSONL exporter: {}", cfg.file);
            #[cfg_attr(not(feature = "replace_parameters"), allow(unused_mut))]
            let mut exporter = JsonlExporter::from_config(cfg);
            #[cfg(feature = "replace_parameters")]
            {
                exporter.normalize = normalize;
            }
            return Ok(Self {
                exporter: Box::new(exporter),
            });
        }

        #[cfg(feature = "sqlite")]
        if let Some(cfg) = &config.exporter.sqlite {
            info!("Using SQLite exporter: {}", cfg.database_url);
            #[cfg_attr(not(feature = "replace_parameters"), allow(unused_mut))]
            let mut exporter = SqliteExporter::from_config(cfg);
            #[cfg(feature = "replace_parameters")]
            {
                exporter.normalize = normalize;
            }
            return Ok(Self {
                exporter: Box::new(exporter),
            });
        }

        Err(Error::Config(ConfigError::NoExporters))
    }

    pub fn initialize(&mut self) -> Result<()> {
        info!("Initializing exporters...");
        self.exporter.initialize()?;
        info!("Exporters initialized");
        Ok(())
    }

    /// 批量导出，直接传 slice，零额外分配
    pub fn export_batch(&mut self, sqllogs: &[Sqllog<'_>]) -> Result<()> {
        self.exporter.export_batch(sqllogs)
    }

    /// 批量导出，同时传入每条记录的 `normalized_sql`（`replace_parameters` 特性专用）
    #[cfg(feature = "replace_parameters")]
    pub fn export_batch_with_normalized(
        &mut self,
        sqllogs: &[Sqllog<'_>],
        normalized: &[Option<String>],
    ) -> Result<()> {
        self.exporter
            .export_batch_with_normalized(sqllogs, normalized)
    }

    pub fn finalize(&mut self) -> Result<()> {
        info!("Finalizing exporters...");
        self.exporter.finalize()?;
        info!("Exporters finished");
        Ok(())
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.exporter.name()
    }

    pub fn log_stats(&self) {
        if let Some(s) = self.exporter.stats_snapshot() {
            info!(
                "Export stats: {} => success: {}, failed: {}, skipped: {} (total: {}){}",
                self.name(),
                s.exported,
                s.failed,
                s.skipped,
                s.total(),
                if s.flush_operations > 0 {
                    format!(
                        " | flushed: {} times (recent {} entries)",
                        s.flush_operations, s.last_flush_size
                    )
                } else {
                    String::new()
                }
            );
        }
    }
}

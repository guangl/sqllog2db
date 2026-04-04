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

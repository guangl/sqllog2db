use super::util::{ensure_parent_dir, strip_ip_prefix};
use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{info, warn};
use serde::Serialize;
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// 零分配借用版记录结构，直接引用 Sqllog 中的数据
#[derive(Debug, Serialize)]
struct JsonlRecord<'a> {
    ts: &'a str,
    ep: u8,
    sess_id: &'a str,
    thrd_id: &'a str,
    username: &'a str,
    trx_id: &'a str,
    statement: &'a str,
    appname: &'a str,
    client_ip: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<&'a str>,
    sql: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    exec_time_ms: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    row_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exec_id: Option<i64>,
}

pub struct JsonlExporter {
    path: PathBuf,
    overwrite: bool,
    append: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
}

impl std::fmt::Debug for JsonlExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonlExporter")
            .field("path", &self.path)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl JsonlExporter {
    #[must_use]
    pub fn new(path: impl AsRef<Path>, overwrite: bool) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            overwrite,
            append: false,
            writer: None,
            stats: ExportStats::new(),
        }
    }

    #[must_use]
    pub fn from_config(config: &crate::config::JsonlExporter) -> Self {
        let mut e = Self::new(&config.file, config.overwrite);
        if config.append {
            e.append = true;
            e.overwrite = false;
        }
        e
    }

    #[inline]
    fn write_record(writer: &mut BufWriter<File>, sqllog: &Sqllog<'_>, path: &Path) -> Result<()> {
        let meta = sqllog.parse_meta();
        let pm = sqllog.parse_performance_metrics();
        let ind = sqllog.parse_indicators();
        let record = JsonlRecord {
            ts: sqllog.ts.as_ref(),
            ep: meta.ep,
            sess_id: meta.sess_id.as_ref(),
            thrd_id: meta.thrd_id.as_ref(),
            username: meta.username.as_ref(),
            trx_id: meta.trxid.as_ref(),
            statement: meta.statement.as_ref(),
            appname: meta.appname.as_ref(),
            client_ip: strip_ip_prefix(meta.client_ip.as_ref()),
            tag: sqllog.tag.as_deref(),
            sql: pm.sql.as_ref(),
            exec_time_ms: ind.as_ref().map(|i| i.exectime),
            row_count: ind.as_ref().map(|i| i.rowcount),
            exec_id: ind.as_ref().map(|i| i.exec_id),
        };
        serde_json::to_writer(&mut *writer, &record).map_err(|e| {
            Error::Export(ExportError::WriteError {
                path: path.to_path_buf(),
                reason: format!("serialize failed: {e}"),
            })
        })?;
        writer.write_all(b"\n").map_err(|e| {
            Error::Export(ExportError::WriteError {
                path: path.to_path_buf(),
                reason: format!("write failed: {e}"),
            })
        })
    }
}

impl Exporter for JsonlExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing JSONL exporter: {}", self.path.display());

        ensure_parent_dir(&self.path).map_err(|e| {
            Error::Export(ExportError::WriteError {
                path: self.path.clone(),
                reason: format!("create dir failed: {e}"),
            })
        })?;

        let file = if self.append {
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
        } else {
            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(self.overwrite)
                .open(&self.path)
        }
        .map_err(|e| {
            Error::Export(ExportError::WriteError {
                path: self.path.clone(),
                reason: format!("open failed: {e}"),
            })
        })?;

        self.writer = Some(BufWriter::with_capacity(16 * 1024 * 1024, file));
        info!("JSONL exporter initialized: {}", self.path.display());
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::WriteError {
                path: self.path.clone(),
                reason: "not initialized".to_string(),
            })
        })?;
        Self::write_record(writer, sqllog, &self.path)?;
        self.stats.record_success();
        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[Sqllog<'_>]) -> Result<()> {
        if sqllogs.is_empty() {
            return Ok(());
        }
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::WriteError {
                path: self.path.clone(),
                reason: "not initialized".to_string(),
            })
        })?;
        for sqllog in sqllogs {
            Self::write_record(writer, sqllog, &self.path)?;
        }
        self.stats.record_success_batch(sqllogs.len());
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush().map_err(|e| {
                Error::Export(ExportError::WriteError {
                    path: self.path.clone(),
                    reason: format!("flush failed: {e}"),
                })
            })?;
            info!(
                "JSONL export finished: {} (success: {}, failed: {})",
                self.path.display(),
                self.stats.exported,
                self.stats.failed
            );
        } else {
            warn!("JSONL exporter not initialized or already finished");
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "JSONL"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl Drop for JsonlExporter {
    fn drop(&mut self) {
        if self.writer.is_some()
            && let Err(e) = self.finalize()
        {
            warn!("JSONL exporter finalization on Drop failed: {e}");
        }
    }
}

use super::{ExportStats, Exporter};
use super::{ensure_parent_dir, strip_ip_prefix};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{info, warn};
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Hex digits for JSON `\uXXXX` escaping of control characters.
const HEX: &[u8; 16] = b"0123456789abcdef";

/// Append `s` as a quoted JSON string, applying full RFC 8259 escaping.
///
/// Use this for user-controlled fields that may contain `"`, `\`, newlines,
/// or other control characters (e.g. `sql`, `username`, `appname`, `tag`).
#[inline]
fn write_json_str(buf: &mut Vec<u8>, s: &str) {
    buf.push(b'"');
    for &byte in s.as_bytes() {
        match byte {
            b'"' => buf.extend_from_slice(b"\\\""),
            b'\\' => buf.extend_from_slice(b"\\\\"),
            b'\n' => buf.extend_from_slice(b"\\n"),
            b'\r' => buf.extend_from_slice(b"\\r"),
            b'\t' => buf.extend_from_slice(b"\\t"),
            0x08 => buf.extend_from_slice(b"\\b"),
            0x0C => buf.extend_from_slice(b"\\f"),
            0x00..=0x1F => {
                buf.extend_from_slice(b"\\u00");
                buf.push(HEX[(byte >> 4) as usize]);
                buf.push(HEX[(byte & 0x0F) as usize]);
            }
            _ => buf.push(byte),
        }
    }
    buf.push(b'"');
}

/// Append `s` as a quoted JSON string **without escaping**.
///
/// Only safe for fields whose format is guaranteed to contain no characters
/// that require JSON escaping: timestamps, hex session/thread/trx IDs,
/// IP addresses, statement IDs.  Avoids the per-byte branch overhead.
#[inline]
fn write_json_str_raw(buf: &mut Vec<u8>, s: &str) {
    buf.push(b'"');
    buf.extend_from_slice(s.as_bytes());
    buf.push(b'"');
}

pub struct JsonlExporter {
    path: PathBuf,
    overwrite: bool,
    append: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
    /// Reused per-record byte buffer — same optimisation as `CsvExporter`.
    line_buf: Vec<u8>,
    itoa_buf: itoa::Buffer,
    float_buf: ryu::Buffer,
    #[cfg(feature = "replace_parameters")]
    pub(super) normalize: bool,
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
            line_buf: Vec::with_capacity(512),
            itoa_buf: itoa::Buffer::new(),
            float_buf: ryu::Buffer::new(),
            #[cfg(feature = "replace_parameters")]
            normalize: true,
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

    /// Format one record into `line_buf` and flush it to `writer` in a single
    /// `write_all` call — zero allocations after the first record.
    #[inline]
    fn write_record(
        line_buf: &mut Vec<u8>,
        itoa_buf: &mut itoa::Buffer,
        float_buf: &mut ryu::Buffer,
        sqllog: &Sqllog<'_>,
        writer: &mut BufWriter<File>,
        path: &Path,
        #[cfg(feature = "replace_parameters")] normalize: bool,
        #[cfg(feature = "replace_parameters")] normalized_sql: Option<&str>,
    ) -> Result<()> {
        let meta = sqllog.parse_meta();
        let pm = sqllog.parse_performance_metrics();
        let ind = sqllog.parse_indicators();

        line_buf.clear();

        // Fields with guaranteed-safe format (timestamps, hex IDs, IPs) —
        // no escaping needed, use direct memcpy.
        line_buf.extend_from_slice(b"{\"ts\":");
        write_json_str_raw(line_buf, sqllog.ts.as_ref());
        line_buf.extend_from_slice(b",\"ep\":");
        line_buf.extend_from_slice(itoa_buf.format(meta.ep).as_bytes());
        line_buf.extend_from_slice(b",\"sess_id\":");
        write_json_str_raw(line_buf, meta.sess_id.as_ref());
        line_buf.extend_from_slice(b",\"thrd_id\":");
        write_json_str_raw(line_buf, meta.thrd_id.as_ref());
        line_buf.extend_from_slice(b",\"username\":");
        write_json_str(line_buf, meta.username.as_ref());
        line_buf.extend_from_slice(b",\"trx_id\":");
        write_json_str_raw(line_buf, meta.trxid.as_ref());
        line_buf.extend_from_slice(b",\"statement\":");
        write_json_str_raw(line_buf, meta.statement.as_ref());
        line_buf.extend_from_slice(b",\"appname\":");
        write_json_str(line_buf, meta.appname.as_ref());
        line_buf.extend_from_slice(b",\"client_ip\":");
        write_json_str_raw(line_buf, strip_ip_prefix(meta.client_ip.as_ref()));

        // Optional: tag (user-controlled, full escaping)
        if let Some(tag) = sqllog.tag.as_deref() {
            line_buf.extend_from_slice(b",\"tag\":");
            write_json_str(line_buf, tag);
        }

        // SQL body (user SQL, full escaping)
        line_buf.extend_from_slice(b",\"sql\":");
        write_json_str(line_buf, pm.sql.as_ref());

        // Optional: performance indicators
        if let Some(ind) = ind {
            line_buf.extend_from_slice(b",\"exec_time_ms\":");
            line_buf.extend_from_slice(float_buf.format(ind.exectime).as_bytes());
            line_buf.extend_from_slice(b",\"row_count\":");
            line_buf.extend_from_slice(itoa_buf.format(i64::from(ind.rowcount)).as_bytes());
            line_buf.extend_from_slice(b",\"exec_id\":");
            line_buf.extend_from_slice(itoa_buf.format(ind.exec_id).as_bytes());
        }

        #[cfg(feature = "replace_parameters")]
        if normalize {
            if let Some(ns) = normalized_sql {
                line_buf.extend_from_slice(b",\"normalized_sql\":");
                write_json_str(line_buf, ns);
            }
        }

        line_buf.extend_from_slice(b"}\n");

        writer.write_all(line_buf).map_err(|e| {
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
        Self::write_record(
            &mut self.line_buf,
            &mut self.itoa_buf,
            &mut self.float_buf,
            sqllog,
            writer,
            &self.path,
            #[cfg(feature = "replace_parameters")]
            self.normalize,
            #[cfg(feature = "replace_parameters")]
            None,
        )?;
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
        #[cfg(feature = "replace_parameters")]
        let normalize = self.normalize;
        for sqllog in sqllogs {
            Self::write_record(
                &mut self.line_buf,
                &mut self.itoa_buf,
                &mut self.float_buf,
                sqllog,
                writer,
                &self.path,
                #[cfg(feature = "replace_parameters")]
                normalize,
                #[cfg(feature = "replace_parameters")]
                None,
            )?;
        }
        self.stats.record_success_batch(sqllogs.len());
        Ok(())
    }

    #[cfg(feature = "replace_parameters")]
    fn export_batch_with_normalized(
        &mut self,
        sqllogs: &[Sqllog<'_>],
        normalized: &[Option<String>],
    ) -> Result<()> {
        if sqllogs.is_empty() {
            return Ok(());
        }
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::WriteError {
                path: self.path.clone(),
                reason: "not initialized".to_string(),
            })
        })?;
        let normalize = self.normalize;
        for (sqllog, ns) in sqllogs.iter().zip(normalized.iter()) {
            Self::write_record(
                &mut self.line_buf,
                &mut self.itoa_buf,
                &mut self.float_buf,
                sqllog,
                writer,
                &self.path,
                normalize,
                ns.as_deref(),
            )?;
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

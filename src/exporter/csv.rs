use super::{ExportStats, Exporter};
use super::{ensure_parent_dir, f32_ms_to_i64, strip_ip_prefix};
use crate::config;
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub struct CsvExporter {
    path: PathBuf,
    overwrite: bool,
    append: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
    itoa_buf: itoa::Buffer,
    line_buf: Vec<u8>,
    pub(super) normalize: bool,
}

impl std::fmt::Debug for CsvExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CsvExporter")
            .field("path", &self.path)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl CsvExporter {
    #[must_use]
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            overwrite: false,
            append: false,
            writer: None,
            stats: ExportStats::new(),
            itoa_buf: itoa::Buffer::new(),
            line_buf: Vec::with_capacity(512),
            normalize: true,
        }
    }

    #[must_use]
    pub fn from_config(config: &config::CsvExporter) -> Self {
        let mut e = Self::new(&config.file);
        if config.append {
            e.append = true;
        } else {
            e.overwrite = config.overwrite;
        }
        e
    }

    /// 将单条记录格式化并写入 writer
    /// 接收各字段的独立可变引用，允许 Rust 同时分开借用 self 的多个字段
    #[inline]
    fn write_record(
        itoa_buf: &mut itoa::Buffer,
        line_buf: &mut Vec<u8>,
        sqllog: &Sqllog<'_>,
        writer: &mut BufWriter<File>,
        path: &Path,
        normalize: bool,
        normalized_sql: Option<&str>,
    ) -> Result<()> {
        let meta = sqllog.parse_meta();
        let pm = sqllog.parse_performance_metrics();

        line_buf.clear();

        line_buf.extend_from_slice(sqllog.ts.as_ref().as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(itoa_buf.format(meta.ep).as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(meta.sess_id.as_ref().as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(meta.thrd_id.as_ref().as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(meta.username.as_ref().as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(meta.trxid.as_ref().as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(meta.statement.as_ref().as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(meta.appname.as_ref().as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(strip_ip_prefix(meta.client_ip.as_ref()).as_bytes());
        line_buf.push(b',');
        if let Some(tag) = &sqllog.tag {
            line_buf.extend_from_slice(tag.as_ref().as_bytes());
        }
        line_buf.push(b',');
        line_buf.push(b'"');
        for &byte in pm.sql.as_bytes() {
            if byte == b'"' {
                line_buf.push(b'"');
            }
            line_buf.push(byte);
        }
        line_buf.push(b'"');
        line_buf.push(b',');
        if pm.exec_id != 0 || pm.exectime > 0.0 {
            line_buf.extend_from_slice(itoa_buf.format(f32_ms_to_i64(pm.exectime)).as_bytes());
            line_buf.push(b',');
            line_buf.extend_from_slice(itoa_buf.format(i64::from(pm.rowcount)).as_bytes());
            line_buf.push(b',');
            line_buf.extend_from_slice(itoa_buf.format(pm.exec_id).as_bytes());
        } else {
            line_buf.extend_from_slice(b",,");
        }

        if normalize {
            line_buf.push(b',');
            if let Some(ns) = normalized_sql {
                line_buf.push(b'"');
                for &byte in ns.as_bytes() {
                    if byte == b'"' {
                        line_buf.push(b'"');
                    }
                    line_buf.push(byte);
                }
                line_buf.push(b'"');
            }
        }

        line_buf.push(b'\n');

        writer.write_all(line_buf).map_err(|e| {
            Error::Export(ExportError::WriteFailed {
                path: path.to_path_buf(),
                reason: format!("write failed: {e}"),
            })
        })
    }
}

impl Exporter for CsvExporter {
    fn initialize(&mut self) -> Result<()> {
        ensure_parent_dir(&self.path).map_err(|e| {
            Error::Export(ExportError::WriteFailed {
                path: self.path.clone(),
                reason: format!("create dir failed: {e}"),
            })
        })?;

        let append_mode = self.append;
        let file_exists = self.path.exists();

        let file = if append_mode {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
        } else {
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(self.overwrite)
                .open(&self.path)
        }
        .map_err(|e| {
            Error::Export(ExportError::WriteFailed {
                path: self.path.clone(),
                reason: format!("open failed: {e}"),
            })
        })?;

        let mut writer = BufWriter::with_capacity(16 * 1024 * 1024, file);

        if !append_mode || !file_exists {
            let header: &[u8] = if self.normalize {
                b"ts,ep,sess_id,thrd_id,username,trx_id,statement,appname,client_ip,tag,sql,exec_time_ms,row_count,exec_id,normalized_sql\n"
            } else {
                b"ts,ep,sess_id,thrd_id,username,trx_id,statement,appname,client_ip,tag,sql,exec_time_ms,row_count,exec_id\n"
            };
            writer.write_all(header).map_err(|e| {
                Error::Export(ExportError::WriteFailed {
                    path: self.path.clone(),
                    reason: format!("write header failed: {e}"),
                })
            })?;
        }

        self.writer = Some(writer);
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::WriteFailed {
                path: self.path.clone(),
                reason: "not initialized".to_string(),
            })
        })?;
        Self::write_record(
            &mut self.itoa_buf,
            &mut self.line_buf,
            sqllog,
            writer,
            &self.path,
            self.normalize,
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
            Error::Export(ExportError::WriteFailed {
                path: self.path.clone(),
                reason: "not initialized".to_string(),
            })
        })?;
        let normalize = self.normalize;
        for sqllog in sqllogs {
            Self::write_record(
                &mut self.itoa_buf,
                &mut self.line_buf,
                sqllog,
                writer,
                &self.path,
                normalize,
                None,
            )?;
        }
        self.stats.record_success_batch(sqllogs.len());
        Ok(())
    }

    fn export_batch_with_normalized(
        &mut self,
        sqllogs: &[Sqllog<'_>],
        normalized: &[Option<String>],
    ) -> Result<()> {
        if sqllogs.is_empty() {
            return Ok(());
        }
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::WriteFailed {
                path: self.path.clone(),
                reason: "not initialized".to_string(),
            })
        })?;
        let normalize = self.normalize;
        for (sqllog, ns) in sqllogs.iter().zip(normalized.iter()) {
            Self::write_record(
                &mut self.itoa_buf,
                &mut self.line_buf,
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
                Error::Export(ExportError::WriteFailed {
                    path: self.path.clone(),
                    reason: format!("flush failed: {e}"),
                })
            })?;
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "CSV"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats)
    }
}

impl Drop for CsvExporter {
    fn drop(&mut self) {
        if self.writer.is_some() {
            let _ = self.finalize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_database_parser_sqllog::LogParser;

    fn write_test_log(path: &std::path::Path, count: usize) {
        use std::fmt::Write as _;
        let mut buf = String::with_capacity(count * 170);
        for i in 0..count {
            writeln!(
                buf,
                "2025-01-15 10:30:28.001 (EP[0] sess:0x{i:04x} user:TESTUSER trxid:{i} stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT * FROM t WHERE id={i}. EXECTIME: {exec}(ms) ROWCOUNT: {rows}(rows) EXEC_ID: {i}.",
                exec = (i * 13) % 1000,
                rows = i % 100,
            ).unwrap();
        }
        std::fs::write(path, buf).unwrap();
    }

    #[test]
    fn test_csv_basic_export() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let outfile = dir.path().join("out.csv");
        write_test_log(&logfile, 5);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();
        assert!(!records.is_empty());

        let mut exporter = CsvExporter::new(&outfile);
        exporter.initialize().unwrap();
        exporter.export_batch(&records).unwrap();
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&outfile).unwrap();
        assert!(content.starts_with("ts,ep,"));
        assert!(content.contains("normalized_sql"));
        // Should have header + 5 data rows
        assert_eq!(content.lines().count(), 6);
    }

    #[test]
    fn test_csv_no_normalize() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let outfile = dir.path().join("out.csv");
        write_test_log(&logfile, 2);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let mut exporter = CsvExporter::new(&outfile);
        exporter.normalize = false;
        exporter.initialize().unwrap();
        exporter.export_batch(&records).unwrap();
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&outfile).unwrap();
        assert!(!content.contains("normalized_sql"));
    }

    #[test]
    fn test_csv_export_batch_with_normalized() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let outfile = dir.path().join("out.csv");
        write_test_log(&logfile, 3);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();
        let normalized: Vec<Option<String>> = records
            .iter()
            .enumerate()
            .map(|(i, _)| Some(format!("SELECT * FROM t WHERE id=?_{i}")))
            .collect();

        let mut exporter = CsvExporter::new(&outfile);
        exporter.normalize = true;
        exporter.initialize().unwrap();
        exporter
            .export_batch_with_normalized(&records, &normalized)
            .unwrap();
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&outfile).unwrap();
        assert!(content.contains("SELECT * FROM t WHERE id=?_0"));
    }

    #[test]
    fn test_csv_append_mode() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let outfile = dir.path().join("out.csv");
        write_test_log(&logfile, 2);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        // First write
        {
            let mut exporter = CsvExporter::from_config(&crate::config::CsvExporter {
                file: outfile.to_string_lossy().into(),
                overwrite: true,
                append: false,
            });
            exporter.initialize().unwrap();
            exporter.export_batch(&records).unwrap();
            exporter.finalize().unwrap();
        }
        let first_count = std::fs::read_to_string(&outfile).unwrap().lines().count();

        // Append second write
        {
            let mut exporter = CsvExporter::from_config(&crate::config::CsvExporter {
                file: outfile.to_string_lossy().into(),
                overwrite: false,
                append: true,
            });
            exporter.initialize().unwrap();
            exporter.export_batch(&records).unwrap();
            exporter.finalize().unwrap();
        }
        let second_count = std::fs::read_to_string(&outfile).unwrap().lines().count();
        // Append adds rows (no header on second write)
        assert!(second_count > first_count);
    }

    #[test]
    fn test_csv_empty_batch_is_noop() {
        let dir = tempfile::TempDir::new().unwrap();
        let outfile = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&outfile);
        exporter.initialize().unwrap();
        exporter.export_batch(&[]).unwrap(); // should not error
        exporter.finalize().unwrap();
        // Only header
        let content = std::fs::read_to_string(&outfile).unwrap();
        assert_eq!(content.lines().count(), 1);
    }
}

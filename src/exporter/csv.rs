use super::{ExportStats, Exporter};
use super::{ensure_parent_dir, f32_ms_to_i64, strip_ip_prefix};
use crate::config;
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::{MetaParts, PerformanceMetrics, Sqllog};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// 将字节序列写入 `buf`，对其中的 `"` 字符进行 CSV 转义（变为 `""`）。
/// 使用 memchr 跳过无引号的大段内容，避免逐字节循环。
#[inline]
fn write_csv_escaped(buf: &mut Vec<u8>, bytes: &[u8]) {
    let mut remaining = bytes;
    while let Some(pos) = memchr::memchr(b'"', remaining) {
        buf.extend_from_slice(&remaining[..=pos]); // 含引号本身
        buf.push(b'"'); // 转义第二个引号
        remaining = &remaining[pos + 1..];
    }
    buf.extend_from_slice(remaining);
}

pub struct CsvExporter {
    path: PathBuf,
    overwrite: bool,
    append: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
    itoa_buf: itoa::Buffer,
    line_buf: Vec<u8>,
    pub(crate) normalize: bool,
    pub(crate) field_mask: crate::features::FieldMask,
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
            // 预分配 2 KiB：覆盖典型日志行（元数据 ~120 B + SQL ~500 B + normalized ~500 B）
            // 避免前几条记录触发 Vec 扩容。clear() 保留容量，运行期自动适配长 SQL。
            line_buf: Vec::with_capacity(2048),
            normalize: true,
            field_mask: crate::features::FieldMask::ALL,
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

    /// 热路径：使用预解析的 `MetaParts` 和 `PerformanceMetrics` 直接格式化并写入。
    /// 接收各字段的独立可变引用，允许 Rust 同时分开借用 self 的多个字段。
    #[inline]
    fn write_record_preparsed(
        itoa_buf: &mut itoa::Buffer,
        line_buf: &mut Vec<u8>,
        sqllog: &Sqllog<'_>,
        meta: &MetaParts<'_>,
        pm: &PerformanceMetrics<'_>,
        writer: &mut BufWriter<File>,
        path: &Path,
        normalize: bool,
        normalized_sql: Option<&str>,
        field_mask: crate::features::FieldMask,
    ) -> Result<()> {
        line_buf.clear();
        let sql_len = pm.sql.len();
        let ns_len = if normalize {
            normalized_sql.map_or(0, str::len)
        } else {
            0
        };
        line_buf.reserve(120 + sql_len + ns_len + 8);

        // 全量掩码快速路径：所有字段直接顺序写入，无分支判断
        if field_mask == crate::features::FieldMask::ALL {
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
            write_csv_escaped(line_buf, pm.sql.as_bytes());
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
                    write_csv_escaped(line_buf, ns.as_bytes());
                    line_buf.push(b'"');
                }
            }
        } else {
            // 投影路径：按 field_mask 选择性写入字段
            let mut need_sep = false;

            macro_rules! w_sep {
                () => {
                    if need_sep {
                        line_buf.push(b',');
                    }
                    need_sep = true;
                };
            }

            if field_mask.is_active(0) {
                w_sep!();
                line_buf.extend_from_slice(sqllog.ts.as_ref().as_bytes());
            }
            if field_mask.is_active(1) {
                w_sep!();
                line_buf.extend_from_slice(itoa_buf.format(meta.ep).as_bytes());
            }
            if field_mask.is_active(2) {
                w_sep!();
                line_buf.extend_from_slice(meta.sess_id.as_ref().as_bytes());
            }
            if field_mask.is_active(3) {
                w_sep!();
                line_buf.extend_from_slice(meta.thrd_id.as_ref().as_bytes());
            }
            if field_mask.is_active(4) {
                w_sep!();
                line_buf.extend_from_slice(meta.username.as_ref().as_bytes());
            }
            if field_mask.is_active(5) {
                w_sep!();
                line_buf.extend_from_slice(meta.trxid.as_ref().as_bytes());
            }
            if field_mask.is_active(6) {
                w_sep!();
                line_buf.extend_from_slice(meta.statement.as_ref().as_bytes());
            }
            if field_mask.is_active(7) {
                w_sep!();
                line_buf.extend_from_slice(meta.appname.as_ref().as_bytes());
            }
            if field_mask.is_active(8) {
                w_sep!();
                line_buf.extend_from_slice(strip_ip_prefix(meta.client_ip.as_ref()).as_bytes());
            }
            if field_mask.is_active(9) {
                w_sep!();
                if let Some(tag) = &sqllog.tag {
                    line_buf.extend_from_slice(tag.as_ref().as_bytes());
                }
            }
            if field_mask.is_active(10) {
                w_sep!();
                line_buf.push(b'"');
                write_csv_escaped(line_buf, pm.sql.as_bytes());
                line_buf.push(b'"');
            }
            let has_metrics = pm.exec_id != 0 || pm.exectime > 0.0;
            if field_mask.is_active(11) {
                w_sep!();
                if has_metrics {
                    line_buf
                        .extend_from_slice(itoa_buf.format(f32_ms_to_i64(pm.exectime)).as_bytes());
                }
            }
            if field_mask.is_active(12) {
                w_sep!();
                if has_metrics {
                    line_buf.extend_from_slice(itoa_buf.format(i64::from(pm.rowcount)).as_bytes());
                }
            }
            if field_mask.is_active(13) {
                w_sep!();
                if has_metrics {
                    line_buf.extend_from_slice(itoa_buf.format(pm.exec_id).as_bytes());
                }
            }
            if normalize && field_mask.is_active(14) {
                w_sep!();
                if let Some(ns) = normalized_sql {
                    line_buf.push(b'"');
                    write_csv_escaped(line_buf, ns.as_bytes());
                    line_buf.push(b'"');
                }
            }
            // 消费 need_sep，避免"最后一次赋值从未被读取"的编译警告
            let _ = need_sep;
        }

        line_buf.push(b'\n');

        writer.write_all(line_buf).map_err(|e| {
            Error::Export(ExportError::WriteFailed {
                path: path.to_path_buf(),
                reason: format!("write failed: {e}"),
            })
        })
    }

    /// 兼容路径：从 `Sqllog` 内部解析再转调热路径（测试/批量导出使用）。
    #[inline]
    fn write_record(
        itoa_buf: &mut itoa::Buffer,
        line_buf: &mut Vec<u8>,
        sqllog: &Sqllog<'_>,
        writer: &mut BufWriter<File>,
        path: &Path,
        normalize: bool,
        normalized_sql: Option<&str>,
        field_mask: crate::features::FieldMask,
    ) -> Result<()> {
        let meta = sqllog.parse_meta();
        let pm = sqllog.parse_performance_metrics();
        Self::write_record_preparsed(
            itoa_buf,
            line_buf,
            sqllog,
            &meta,
            &pm,
            writer,
            path,
            normalize,
            normalized_sql,
            field_mask,
        )
    }

    /// 根据 `field_mask` 和 `normalize` 标志生成 CSV 头行
    fn build_header(&self) -> Vec<u8> {
        use crate::features::FIELD_NAMES;
        let mut header = Vec::with_capacity(128);
        let mut first = true;
        for (i, name) in FIELD_NAMES.iter().enumerate() {
            // 字段 14 (normalized_sql) 在 normalize=false 时跳过
            if i == 14 && !self.normalize {
                continue;
            }
            if self.field_mask.is_active(i) {
                if !first {
                    header.push(b',');
                }
                first = false;
                header.extend_from_slice(name.as_bytes());
            }
        }
        header.push(b'\n');
        header
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
            let header = self.build_header();
            writer.write_all(&header).map_err(|e| {
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
            self.field_mask,
        )?;
        self.stats.record_success();
        Ok(())
    }

    fn export_one_normalized(
        &mut self,
        sqllog: &Sqllog<'_>,
        normalized: Option<&str>,
    ) -> Result<()> {
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
            normalized,
            self.field_mask,
        )?;
        self.stats.record_success();
        Ok(())
    }

    fn export_one_preparsed(
        &mut self,
        sqllog: &Sqllog<'_>,
        meta: &MetaParts<'_>,
        pm: &PerformanceMetrics<'_>,
        normalized: Option<&str>,
    ) -> Result<()> {
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::WriteFailed {
                path: self.path.clone(),
                reason: "not initialized".to_string(),
            })
        })?;
        Self::write_record_preparsed(
            &mut self.itoa_buf,
            &mut self.line_buf,
            sqllog,
            meta,
            pm,
            writer,
            &self.path,
            self.normalize,
            normalized,
            self.field_mask,
        )?;
        self.stats.record_success();
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
        for r in &records {
            exporter.export_one_normalized(r, None).unwrap();
        }
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
        for r in &records {
            exporter.export_one_normalized(r, None).unwrap();
        }
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&outfile).unwrap();
        assert!(!content.contains("normalized_sql"));
    }

    #[test]
    fn test_csv_export_with_normalized() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let outfile = dir.path().join("out.csv");
        write_test_log(&logfile, 3);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let mut exporter = CsvExporter::new(&outfile);
        exporter.normalize = true;
        exporter.initialize().unwrap();
        for (i, r) in records.iter().enumerate() {
            let ns = format!("SELECT * FROM t WHERE id=?_{i}");
            exporter.export_one_normalized(r, Some(&ns)).unwrap();
        }
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
            for r in &records {
                exporter.export_one_normalized(r, None).unwrap();
            }
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
            for r in &records {
                exporter.export_one_normalized(r, None).unwrap();
            }
            exporter.finalize().unwrap();
        }
        let second_count = std::fs::read_to_string(&outfile).unwrap().lines().count();
        // Append adds rows (no header on second write)
        assert!(second_count > first_count);
    }

    #[test]
    fn test_csv_empty_export_is_noop() {
        let dir = tempfile::TempDir::new().unwrap();
        let outfile = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&outfile);
        exporter.initialize().unwrap();
        exporter.finalize().unwrap();
        // Only header
        let content = std::fs::read_to_string(&outfile).unwrap();
        assert_eq!(content.lines().count(), 1);
    }

    #[test]
    fn test_csv_debug_format() {
        let exporter = CsvExporter::new("/tmp/debug.csv");
        let s = format!("{exporter:?}");
        assert!(s.contains("CsvExporter"));
    }

    #[test]
    fn test_csv_export_method() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let outfile = dir.path().join("out.csv");
        write_test_log(&logfile, 3);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let mut exporter = CsvExporter::new(&outfile);
        exporter.initialize().unwrap();
        for r in &records {
            // Use export() directly instead of export_one_normalized
            exporter.export(r).unwrap();
        }
        exporter.finalize().unwrap();

        let lines = std::fs::read_to_string(&outfile).unwrap().lines().count();
        assert_eq!(lines, records.len() + 1); // data + header
    }

    #[test]
    fn test_csv_stats_snapshot() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let outfile = dir.path().join("out.csv");
        write_test_log(&logfile, 5);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let mut exporter = CsvExporter::new(&outfile);
        exporter.initialize().unwrap();
        for r in &records {
            exporter.export(r).unwrap();
        }
        let snap = exporter.stats_snapshot().unwrap();
        assert_eq!(snap.exported, 5);
        exporter.finalize().unwrap();
    }

    #[test]
    fn test_write_csv_escaped_with_quotes() {
        // write_csv_escaped handles '"' characters by doubling them
        let mut buf = Vec::new();
        write_csv_escaped(&mut buf, b"say \"hello\"");
        assert_eq!(buf, b"say \"\"hello\"\"");
    }

    #[test]
    fn test_write_csv_escaped_no_quotes() {
        let mut buf = Vec::new();
        write_csv_escaped(&mut buf, b"no quotes here");
        assert_eq!(buf, b"no quotes here");
    }

    #[test]
    fn test_csv_from_config() {
        use crate::config;
        let cfg = config::CsvExporter {
            file: "/tmp/cfg.csv".to_string(),
            overwrite: true,
            append: false,
        };
        let exporter = CsvExporter::from_config(&cfg);
        let s = format!("{exporter:?}");
        assert!(s.contains("CsvExporter"));
    }
}

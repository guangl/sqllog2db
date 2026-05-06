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
    pub(crate) ordered_indices: Vec<usize>,
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
            ordered_indices: (0..crate::features::FIELD_NAMES.len()).collect(),
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
    pub(crate) fn write_record_preparsed(
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
        ordered_indices: &[usize],
    ) -> Result<()> {
        line_buf.clear();
        let sql_len = pm.sql.len();
        let ns_len = if normalize {
            normalized_sql.map_or(0, str::len)
        } else {
            0
        };
        let needed = 128 + sql_len + ns_len;
        if line_buf.capacity() < needed {
            line_buf.reserve(needed - line_buf.len());
        }

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
            // 投影路径：按 ordered_indices 指定的字段顺序写入
            let mut need_sep = false;

            macro_rules! w_sep {
                () => {
                    if need_sep {
                        line_buf.push(b',');
                    }
                    need_sep = true;
                };
            }

            let has_metrics = pm.exec_id != 0 || pm.exectime > 0.0;
            for &idx in ordered_indices {
                match idx {
                    0 => {
                        w_sep!();
                        line_buf.extend_from_slice(sqllog.ts.as_ref().as_bytes());
                    }
                    1 => {
                        w_sep!();
                        line_buf.extend_from_slice(itoa_buf.format(meta.ep).as_bytes());
                    }
                    2 => {
                        w_sep!();
                        line_buf.extend_from_slice(meta.sess_id.as_ref().as_bytes());
                    }
                    3 => {
                        w_sep!();
                        line_buf.extend_from_slice(meta.thrd_id.as_ref().as_bytes());
                    }
                    4 => {
                        w_sep!();
                        line_buf.extend_from_slice(meta.username.as_ref().as_bytes());
                    }
                    5 => {
                        w_sep!();
                        line_buf.extend_from_slice(meta.trxid.as_ref().as_bytes());
                    }
                    6 => {
                        w_sep!();
                        line_buf.extend_from_slice(meta.statement.as_ref().as_bytes());
                    }
                    7 => {
                        w_sep!();
                        line_buf.extend_from_slice(meta.appname.as_ref().as_bytes());
                    }
                    8 => {
                        w_sep!();
                        line_buf
                            .extend_from_slice(strip_ip_prefix(meta.client_ip.as_ref()).as_bytes());
                    }
                    9 => {
                        w_sep!();
                        if let Some(tag) = &sqllog.tag {
                            line_buf.extend_from_slice(tag.as_ref().as_bytes());
                        }
                    }
                    10 => {
                        w_sep!();
                        line_buf.push(b'"');
                        write_csv_escaped(line_buf, pm.sql.as_bytes());
                        line_buf.push(b'"');
                    }
                    11 => {
                        w_sep!();
                        if has_metrics {
                            line_buf.extend_from_slice(
                                itoa_buf.format(f32_ms_to_i64(pm.exectime)).as_bytes(),
                            );
                        }
                    }
                    12 => {
                        w_sep!();
                        if has_metrics {
                            line_buf.extend_from_slice(
                                itoa_buf.format(i64::from(pm.rowcount)).as_bytes(),
                            );
                        }
                    }
                    13 => {
                        w_sep!();
                        if has_metrics {
                            line_buf.extend_from_slice(itoa_buf.format(pm.exec_id).as_bytes());
                        }
                    }
                    14 => {
                        // D-03：normalize=false 时跳过 normalized_sql，与 header 逻辑一致
                        if normalize {
                            w_sep!();
                            if let Some(ns) = normalized_sql {
                                line_buf.push(b'"');
                                write_csv_escaped(line_buf, ns.as_bytes());
                                line_buf.push(b'"');
                            }
                        }
                    }
                    _ => {}
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
        ordered_indices: &[usize],
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
            ordered_indices,
        )
    }

    /// 根据 `ordered_indices` 和 `normalize` 标志生成 CSV 头行
    fn build_header(&self) -> Vec<u8> {
        use crate::features::FIELD_NAMES;
        let mut header = Vec::with_capacity(128);
        let mut first = true;
        for &idx in &self.ordered_indices {
            // idx 14 (normalized_sql) 在 normalize=false 时跳过（与全量路径行为一致）
            if idx == 14 && !self.normalize {
                continue;
            }
            if !first {
                header.push(b',');
            }
            first = false;
            header.extend_from_slice(FIELD_NAMES[idx].as_bytes());
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
            &self.ordered_indices,
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
            &self.ordered_indices,
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
            &self.ordered_indices,
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
                ..crate::config::CsvExporter::default()
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
                ..crate::config::CsvExporter::default()
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
            ..config::CsvExporter::default()
        };
        let exporter = CsvExporter::from_config(&cfg);
        let s = format!("{exporter:?}");
        assert!(s.contains("CsvExporter"));
    }

    #[test]
    fn test_csv_header_field_order() {
        use crate::features::FieldMask;
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&path);
        exporter.field_mask =
            FieldMask::from_names(&["sql".to_string(), "username".to_string()]).unwrap();
        exporter.ordered_indices = vec![10, 4]; // sql=10, username=4
        exporter.initialize().unwrap();
        exporter.finalize().unwrap(); // flush BufWriter before reading
        let content = std::fs::read_to_string(&path).unwrap();
        let header_line = content.lines().next().unwrap();
        assert_eq!(header_line, "sql,username");
    }

    #[test]
    fn test_csv_header_full_order() {
        use crate::features::FIELD_NAMES;
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&path);
        exporter.normalize = true;
        // ordered_indices 默认全量 [0..14]，无需修改
        exporter.initialize().unwrap();
        exporter.finalize().unwrap(); // flush BufWriter before reading
        let content = std::fs::read_to_string(&path).unwrap();
        let header_line = content.lines().next().unwrap();
        let expected: Vec<&str> = FIELD_NAMES.to_vec();
        assert_eq!(header_line, expected.join(","));
    }

    #[test]
    fn test_csv_header_no_normalized_sql_when_normalize_false() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&path);
        exporter.normalize = false;
        // ordered_indices 默认全量，但 idx=14 应被跳过
        exporter.initialize().unwrap();
        exporter.finalize().unwrap(); // flush BufWriter before reading
        let content = std::fs::read_to_string(&path).unwrap();
        let header_line = content.lines().next().unwrap();
        assert!(!header_line.contains("normalized_sql"));
        assert!(header_line.contains("sql")); // idx=10 的 "sql" 字段仍存在
    }

    #[test]
    fn test_csv_field_order() {
        // 验证数据行按 ordered_indices=[10,4] 顺序输出（sql, username 两列）
        use crate::features::FieldMask;

        let dir = tempfile::TempDir::new().unwrap();
        let log = dir.path().join("t.log");
        std::fs::write(
            &log,
            "2025-01-15 10:30:28.001 (EP[0] sess:0x0001 user:testuser trxid:1 stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT 1. EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n",
        )
        .unwrap();

        let out = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&out);
        exporter.normalize = false;
        exporter.field_mask =
            FieldMask::from_names(&["sql".to_string(), "username".to_string()]).unwrap();
        exporter.ordered_indices = vec![10, 4]; // sql=10, username=4
        exporter.initialize().unwrap();

        let parser = LogParser::from_path(log.to_str().unwrap()).unwrap();
        for record in parser.iter().flatten() {
            exporter.export(&record).unwrap();
        }
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&out).unwrap();
        let mut lines = content.lines();
        let header = lines.next().unwrap();
        let data = lines.next().unwrap();

        assert_eq!(header, "sql,username");
        // 数据行第一列是 sql 内容（含引号），第二列是 username=testuser
        assert!(data.ends_with(",testuser"), "data line: {data}");
    }

    #[test]
    fn test_csv_field_order_normalized_sql_skipped_when_normalize_false() {
        use crate::features::FieldMask;

        let dir = tempfile::TempDir::new().unwrap();
        let log = dir.path().join("t.log");
        std::fs::write(
            &log,
            "2025-01-15 10:30:28.001 (EP[0] sess:0x0001 user:U trxid:1 stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT 1. EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n",
        )
        .unwrap();

        let out = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&out);
        exporter.normalize = false;
        // ordered_indices 含 14（normalized_sql），但 normalize=false 时应跳过（D-03）
        exporter.ordered_indices = vec![10, 14]; // sql, normalized_sql（后者被跳过）
        exporter.field_mask = FieldMask::from_names(&["sql".to_string()]).unwrap();
        exporter.initialize().unwrap();

        let parser = LogParser::from_path(log.to_str().unwrap()).unwrap();
        for record in parser.iter().flatten() {
            exporter.export(&record).unwrap();
        }
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&out).unwrap();
        let header = content.lines().next().unwrap();
        // normalize=false 时 normalized_sql 不出现在 header 中
        assert!(!header.contains("normalized_sql"), "header: {header}");
    }

    #[test]
    fn test_csv_reserve_boundary_short_sql() {
        // 回归：极短 SQL（10 字节级）触发 reserve 路径，输出格式必须完整
        let dir = tempfile::TempDir::new().unwrap();
        let log = dir.path().join("short.log");
        std::fs::write(
            &log,
            "2025-01-15 10:30:28.001 (EP[0] sess:0x0001 user:U trxid:1 stmt:0x1 appname:A ip:10.0.0.1) [SEL] SELECT 1. EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n",
        ).unwrap();

        let out = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&out);
        exporter.initialize().unwrap();

        let parser = LogParser::from_path(log.to_str().unwrap()).unwrap();
        for record in parser.iter().flatten() {
            exporter.export(&record).unwrap();
        }
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&out).unwrap();
        // header + 1 data row
        assert_eq!(content.lines().count(), 2);
        let data = content.lines().nth(1).unwrap();
        assert!(data.contains("\"SELECT 1"), "data row: {data}");
    }

    #[test]
    fn test_csv_reserve_boundary_long_sql() {
        // 回归：长 SQL（>2KB）触发 reserve 扩容路径，line_buf 容量正确扩展
        let dir = tempfile::TempDir::new().unwrap();
        let log = dir.path().join("long.log");
        let big_sql = "x".repeat(4096);
        let line = format!(
            "2025-01-15 10:30:28.001 (EP[0] sess:0x0001 user:U trxid:1 stmt:0x1 appname:A ip:10.0.0.1) [SEL] SELECT '{big_sql}'. EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n"
        );
        std::fs::write(&log, &line).unwrap();

        let out = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&out);
        exporter.initialize().unwrap();

        let parser = LogParser::from_path(log.to_str().unwrap()).unwrap();
        for record in parser.iter().flatten() {
            exporter.export(&record).unwrap();
        }
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&out).unwrap();
        assert_eq!(content.lines().count(), 2);
        // 长 SQL 在数据行内完整存在
        assert!(content.contains(&big_sql), "long SQL missing from output");
    }
}

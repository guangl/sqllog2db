use super::{ExportStats, Exporter};
use super::{ensure_parent_dir, f32_ms_to_i64, strip_ip_prefix};
use crate::config;
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::{MetaParts, PerformanceMetrics, Sqllog};
use log::info;
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

/// 根据主 CSV 路径推导伴随文件路径（D-09）：`<stem>_templates.csv`
fn build_companion_path(base_path: &Path) -> PathBuf {
    let stem = base_path.file_stem().unwrap_or_default();
    base_path.with_file_name(format!("{}_templates.csv", stem.to_string_lossy()))
}

/// 将单行模板统计序列化到 `buf`（`template_key` 含双引号包裹 + CSV 转义，数值用 itoa）
fn format_companion_row(
    buf: &mut Vec<u8>,
    itoa_buf: &mut itoa::Buffer,
    s: &crate::features::TemplateStats,
) {
    buf.clear();
    buf.push(b'"');
    write_csv_escaped(buf, s.template_key.as_bytes());
    buf.push(b'"');
    buf.push(b',');
    buf.extend_from_slice(itoa_buf.format(s.count).as_bytes());
    buf.push(b',');
    buf.extend_from_slice(itoa_buf.format(s.avg_us).as_bytes());
    buf.push(b',');
    buf.extend_from_slice(itoa_buf.format(s.min_us).as_bytes());
    buf.push(b',');
    buf.extend_from_slice(itoa_buf.format(s.max_us).as_bytes());
    buf.push(b',');
    buf.extend_from_slice(itoa_buf.format(s.p50_us).as_bytes());
    buf.push(b',');
    buf.extend_from_slice(itoa_buf.format(s.p95_us).as_bytes());
    buf.push(b',');
    buf.extend_from_slice(itoa_buf.format(s.p99_us).as_bytes());
    buf.push(b',');
    buf.push(b'"');
    write_csv_escaped(buf, s.first_seen.as_bytes());
    buf.push(b'"');
    buf.push(b',');
    buf.push(b'"');
    write_csv_escaped(buf, s.last_seen.as_bytes());
    buf.push(b'"');
    buf.push(b'\n');
}

/// 将 I/O 错误包装为 `ExportError::WriteFailed`
#[inline]
fn io_err(path: &Path, reason: String) -> Error {
    Error::Export(ExportError::WriteFailed {
        path: path.to_path_buf(),
        reason,
    })
}

/// 将模板统计写入伴随 CSV 文件（D-10：始终覆盖写入）
fn write_companion_rows(path: &Path, stats: &[crate::features::TemplateStats]) -> Result<()> {
    ensure_parent_dir(path).map_err(|e| io_err(path, format!("create dir failed: {e}")))?;
    let file =
        File::create(path).map_err(|e| io_err(path, format!("create companion failed: {e}")))?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(
            b"template_key,count,avg_us,min_us,max_us,p50_us,p95_us,p99_us,first_seen,last_seen\n",
        )
        .map_err(|e| io_err(path, format!("write header failed: {e}")))?;
    let mut itoa_buf = itoa::Buffer::new();
    let mut line_buf: Vec<u8> = Vec::with_capacity(512);
    for s in stats {
        format_companion_row(&mut line_buf, &mut itoa_buf, s);
        writer
            .write_all(&line_buf)
            .map_err(|e| io_err(path, format!("write row failed: {e}")))?;
    }
    writer
        .flush()
        .map_err(|e| io_err(path, format!("flush failed: {e}")))?;
    Ok(())
}

#[allow(clippy::struct_excessive_bools)]
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
    /// 是否在输出中包含性能指标列（`exec_time_ms`/`row_count`/`exec_id`）。
    /// 关闭时 header 和数据行都跳过这三列；调用方（`cli/run.rs`）也应跳过 `parse_performance_metrics()`。
    pub(crate) include_performance_metrics: bool,
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
            include_performance_metrics: true,
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
        e.include_performance_metrics = config.include_performance_metrics;
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
        include_performance_metrics: bool,
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
            if include_performance_metrics {
                line_buf.push(b',');
                if pm.exec_id != 0 || pm.exectime > 0.0 {
                    line_buf
                        .extend_from_slice(itoa_buf.format(f32_ms_to_i64(pm.exectime)).as_bytes());
                    line_buf.push(b',');
                    line_buf.extend_from_slice(itoa_buf.format(i64::from(pm.rowcount)).as_bytes());
                    line_buf.push(b',');
                    line_buf.extend_from_slice(itoa_buf.format(pm.exec_id).as_bytes());
                } else {
                    line_buf.extend_from_slice(b",,");
                }
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
                        if !include_performance_metrics {
                            continue;
                        }
                        w_sep!();
                        if has_metrics {
                            line_buf.extend_from_slice(
                                itoa_buf.format(f32_ms_to_i64(pm.exectime)).as_bytes(),
                            );
                        }
                    }
                    12 => {
                        if !include_performance_metrics {
                            continue;
                        }
                        w_sep!();
                        if has_metrics {
                            line_buf.extend_from_slice(
                                itoa_buf.format(i64::from(pm.rowcount)).as_bytes(),
                            );
                        }
                    }
                    13 => {
                        if !include_performance_metrics {
                            continue;
                        }
                        w_sep!();
                        if has_metrics {
                            line_buf.extend_from_slice(itoa_buf.format(pm.exec_id).as_bytes());
                        }
                    }
                    // D-03：normalize=false 时跳过 normalized_sql，与 header 逻辑一致
                    14 if normalize => {
                        w_sep!();
                        if let Some(ns) = normalized_sql {
                            line_buf.push(b'"');
                            write_csv_escaped(line_buf, ns.as_bytes());
                            line_buf.push(b'"');
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
        include_performance_metrics: bool,
    ) -> Result<()> {
        let meta = sqllog.parse_meta();
        let pm = if include_performance_metrics {
            sqllog.parse_performance_metrics()
        } else {
            PerformanceMetrics {
                sql: sqllog.body(),
                exectime: 0.0,
                rowcount: 0,
                exec_id: 0,
            }
        };
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
            include_performance_metrics,
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
            // idx 11/12/13 (exectime/rowcount/exec_id) 在 include_performance_metrics=false 时跳过（D-05/D-06）
            if matches!(idx, 11..=13) && !self.include_performance_metrics {
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
            self.include_performance_metrics,
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
            self.include_performance_metrics,
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
            self.include_performance_metrics,
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

    fn write_template_stats(
        &mut self,
        stats: &[crate::features::TemplateStats],
        final_path: Option<&std::path::Path>,
    ) -> Result<()> {
        let base_path: &Path = final_path.unwrap_or(self.path.as_path());
        let companion = build_companion_path(base_path);
        write_companion_rows(&companion, stats)?;
        info!("Template companion CSV written: {}", companion.display());
        Ok(())
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

    #[test]
    fn test_csv_header_skips_pm_when_disabled() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("out.csv");
        let mut exporter = CsvExporter::new(&path);
        exporter.include_performance_metrics = false;
        exporter.initialize().unwrap();
        exporter.finalize().unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let header = content.lines().next().unwrap();
        assert!(!header.contains("exec_time_ms"), "header: {header}");
        assert!(!header.contains("row_count"), "header: {header}");
        assert!(!header.contains("exec_id"), "header: {header}");
        assert!(header.contains("sql"), "sql column should remain");
    }

    #[test]
    fn test_csv_data_row_skips_pm_when_disabled() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let outfile = dir.path().join("out.csv");
        write_test_log(&logfile, 3);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let mut exporter = CsvExporter::new(&outfile);
        exporter.include_performance_metrics = false;
        exporter.initialize().unwrap();
        for r in &records {
            exporter.export(r).unwrap();
        }
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&outfile).unwrap();
        let header = content.lines().next().unwrap();
        let header_cols = header.split(',').count();
        // 关闭性能指标后 header 列数 == 全量列数 - 3
        // 全量含 normalized_sql：15；关闭性能指标后剩 12 列
        assert_eq!(header_cols, 12, "header: {header}");
        // 数据行列数也应为 12（注意 SQL 列含双引号但不含逗号）
        for line in content.lines().skip(1) {
            let cols = line.split(',').count();
            assert_eq!(cols, 12, "data row: {line}");
        }
    }

    #[test]
    fn test_csv_default_include_pm_true_keeps_existing_behavior() {
        // 默认（include_performance_metrics=true）输出与历史行为一致
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let outfile = dir.path().join("out.csv");
        write_test_log(&logfile, 2);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let mut exporter = CsvExporter::new(&outfile);
        // 不显式设置 include_performance_metrics，应为默认 true
        exporter.initialize().unwrap();
        for r in &records {
            exporter.export(r).unwrap();
        }
        exporter.finalize().unwrap();

        let content = std::fs::read_to_string(&outfile).unwrap();
        let header = content.lines().next().unwrap();
        assert!(header.contains("exec_time_ms"));
        assert!(header.contains("row_count"));
        assert!(header.contains("exec_id"));
    }

    /// TMPL-04-B：验证 `write_template_stats` 写入伴随文件，含 CSV 转义
    #[test]
    fn test_csv_write_template_stats() {
        let dir = tempfile::TempDir::new().unwrap();
        let outfile = dir.path().join("output.csv");

        let mut exporter = CsvExporter::new(&outfile);
        exporter.initialize().unwrap();
        exporter.finalize().unwrap();

        // 第一个 template_key 含逗号，需转义
        let stats = vec![
            crate::features::TemplateStats {
                template_key: r#"SELECT * FROM t WHERE name = "John", age = ?"#.to_string(),
                count: 42,
                avg_us: 150,
                min_us: 10,
                max_us: 500,
                p50_us: 120,
                p95_us: 400,
                p99_us: 480,
                first_seen: "2025-01-01 00:00:00".to_string(),
                last_seen: "2025-01-01 12:00:00".to_string(),
            },
            crate::features::TemplateStats {
                template_key: "INSERT INTO t VALUES (?)".to_string(),
                count: 7,
                avg_us: 80,
                min_us: 5,
                max_us: 200,
                p50_us: 70,
                p95_us: 180,
                p99_us: 195,
                first_seen: "2025-01-01 01:00:00".to_string(),
                last_seen: "2025-01-01 11:00:00".to_string(),
            },
        ];

        exporter.write_template_stats(&stats, None).unwrap();

        let companion = dir.path().join("output_templates.csv");
        assert!(companion.exists(), "伴随文件应存在");

        let content = std::fs::read_to_string(&companion).unwrap();
        let mut lines = content.lines();

        // 验证表头精确匹配
        let header = lines.next().unwrap();
        assert_eq!(
            header,
            "template_key,count,avg_us,min_us,max_us,p50_us,p95_us,p99_us,first_seen,last_seen"
        );

        // 验证数据行数量 = 2
        let data_lines: Vec<&str> = lines.collect();
        assert_eq!(data_lines.len(), 2);

        // 验证第一行：template_key 含引号+逗号，应被双引号包裹且引号转义
        let first_row = data_lines[0];
        assert!(
            first_row.starts_with('"'),
            "含特殊字符的 template_key 应被双引号包裹"
        );
        assert!(
            first_row.contains("\"\"John\"\""),
            "引号应被转义为 \"\", row: {first_row}"
        );

        // 验证数值字段可直接 parse
        // 格式："<key>",count,avg_us,...,p99_us,"first_seen","last_seen"
        // template_key 以 ," 开头，找到 key 结束引号后提取数值部分（不含末尾时间戳字段）
        // 第一个 template_key 是 `"SELECT * FROM t WHERE name = ""John"", age = ?"`,
        // 末尾 `?"` 即 key 结束。之后的格式为 ,count,avg,...,p99_us,"first_seen","last_seen"
        // 用逗号分割，提取第 1、2 个数值字段（count, avg_us）。
        let after_key = {
            // key 的结束引号后紧跟 `,count`，key 内部含有 `?` 后接 `"` 的组合。
            // 找到第一组 `,"` 之后的第一个逗号分隔边界——更稳妥地直接按 CSV 字段拆分。
            // 简化做法：用 `","` 定位 key 的末尾（key 以 `?"` 结尾，其后紧随 `,42,`）。
            // key 末尾实际是 `= ?"`, 故找 `?"` 再跳过一个 `,` 最简单。
            let end_marker = "?\"";
            let pos = first_row.find(end_marker).expect("应找到 key 结尾标记");
            &first_row[pos + end_marker.len()..]
        };
        let nums: Vec<&str> = after_key.trim_start_matches(',').split(',').collect();
        assert_eq!(nums[0].parse::<u64>().unwrap(), 42u64);
        assert_eq!(nums[1].parse::<u64>().unwrap(), 150u64);
        // first_seen 和 last_seen 应被双引号包裹（nums[7] 和 nums[8]）
        assert_eq!(nums[7], "\"2025-01-01 00:00:00\"");
        assert_eq!(nums[8], "\"2025-01-01 12:00:00\"");
    }

    /// TMPL-04-H：验证 `final_path` 覆盖路径推导（D-09）
    #[test]
    fn test_parallel_csv_companion_file() {
        let dir = tempfile::TempDir::new().unwrap();
        // self.path = output.csv（并行前的占位路径）
        let self_path = dir.path().join("output.csv");
        // final_path = actual_output.csv（并行实际写入路径）
        let final_path = dir.path().join("actual_output.csv");

        let mut exporter = CsvExporter::new(&self_path);

        let stats = vec![crate::features::TemplateStats {
            template_key: "SELECT 1".to_string(),
            count: 1,
            avg_us: 100,
            min_us: 10,
            max_us: 200,
            p50_us: 90,
            p95_us: 180,
            p99_us: 195,
            first_seen: "2025-01-01 00:00:00".to_string(),
            last_seen: "2025-01-01 01:00:00".to_string(),
        }];

        exporter
            .write_template_stats(&stats, Some(final_path.as_path()))
            .unwrap();

        // 伴随文件应在 actual_output_templates.csv，而非 output_templates.csv
        let expected_companion = dir.path().join("actual_output_templates.csv");
        let wrong_companion = dir.path().join("output_templates.csv");

        assert!(
            expected_companion.exists(),
            "伴随文件应在 actual_output_templates.csv"
        );
        assert!(
            !wrong_companion.exists(),
            "output_templates.csv 不应存在（应使用 final_path 推导）"
        );
    }
}

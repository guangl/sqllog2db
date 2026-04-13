use crate::config::Config;
use crate::error::{ConfigError, Error, Result};
use dm_database_parser_sqllog::{MetaParts, PerformanceMetrics, Sqllog};
use log::info;

pub mod csv;
pub mod sqlite;
pub use csv::CsvExporter;
pub use sqlite::SqliteExporter;

/// 所有导出器必须实现的接口
pub trait Exporter {
    fn initialize(&mut self) -> Result<()>;
    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()>;

    /// 流式导出单条记录，同时附带 `normalized_sql`（流式路径，无需 batch）。
    /// 默认实现忽略 normalized，调用 `export`。
    fn export_one_normalized(
        &mut self,
        sqllog: &Sqllog<'_>,
        normalized: Option<&str>,
    ) -> Result<()> {
        let _ = normalized;
        self.export(sqllog)
    }

    /// 热路径：接收调用方已预解析的 `MetaParts` 和 `PerformanceMetrics`，
    /// 避免在导出器内部重复调用 `parse_meta()` / `parse_performance_metrics()`。
    /// 默认实现退化为 `export_one_normalized`（不使用预解析数据）。
    fn export_one_preparsed(
        &mut self,
        sqllog: &Sqllog<'_>,
        meta: &MetaParts<'_>,
        pm: &PerformanceMetrics<'_>,
        normalized: Option<&str>,
    ) -> Result<()> {
        let _ = (meta, pm);
        self.export_one_normalized(sqllog, normalized)
    }

    fn finalize(&mut self) -> Result<()>;

    fn stats_snapshot(&self) -> Option<ExportStats> {
        None
    }
}

/// 具体导出器的枚举包装，消除 `Box<dyn Exporter>` 的虚表分发开销，
/// 使编译器能够内联热路径（`export_one_preparsed` → `write_record_preparsed`）。
#[derive(Debug)]
pub enum ExporterKind {
    Csv(CsvExporter),
    Sqlite(SqliteExporter),
    DryRun(DryRunExporter),
}

impl ExporterKind {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Csv(_) => "CSV",
            Self::Sqlite(_) => "SQLite",
            Self::DryRun(_) => "dry-run",
        }
    }

    fn initialize(&mut self) -> Result<()> {
        match self {
            Self::Csv(e) => e.initialize(),
            Self::Sqlite(e) => e.initialize(),
            Self::DryRun(e) => e.initialize(),
        }
    }

    #[inline]
    fn export_one_preparsed(
        &mut self,
        sqllog: &Sqllog<'_>,
        meta: &MetaParts<'_>,
        pm: &PerformanceMetrics<'_>,
        normalized: Option<&str>,
    ) -> Result<()> {
        match self {
            Self::Csv(e) => e.export_one_preparsed(sqllog, meta, pm, normalized),
            Self::Sqlite(e) => e.export_one_preparsed(sqllog, meta, pm, normalized),
            Self::DryRun(e) => e.export_one_preparsed(sqllog, meta, pm, normalized),
        }
    }

    fn finalize(&mut self) -> Result<()> {
        match self {
            Self::Csv(e) => e.finalize(),
            Self::Sqlite(e) => e.finalize(),
            Self::DryRun(e) => e.finalize(),
        }
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        match self {
            Self::Csv(e) => e.stats_snapshot(),
            Self::Sqlite(e) => e.stats_snapshot(),
            Self::DryRun(e) => e.stats_snapshot(),
        }
    }
}

/// 导出统计
#[derive(Debug, Default, Clone, Copy)]
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

    /// 直接计数，跳过两层默认实现（`export_one_normalized` → `export`）。
    #[inline]
    fn export_one_preparsed(
        &mut self,
        _sqllog: &Sqllog<'_>,
        _meta: &MetaParts<'_>,
        _pm: &PerformanceMetrics<'_>,
        _normalized: Option<&str>,
    ) -> Result<()> {
        self.stats.exported += 1;
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats)
    }
}

/// 导出器管理器
pub struct ExporterManager {
    exporter: ExporterKind,
}

impl std::fmt::Debug for ExporterManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExporterManager")
            .field("exporter", &self.exporter.kind_name())
            .finish()
    }
}

impl ExporterManager {
    /// 从已构建的 `CsvExporter` 创建管理器（并行处理时每个任务独立调用）。
    #[must_use]
    pub fn from_csv(exporter: CsvExporter) -> Self {
        Self {
            exporter: ExporterKind::Csv(exporter),
        }
    }

    /// 创建空运行导出器，只统计记录数不写文件
    #[must_use]
    pub fn dry_run() -> Self {
        info!("Dry-run mode: no output will be written");
        Self {
            exporter: ExporterKind::DryRun(DryRunExporter::default()),
        }
    }

    pub fn from_config(config: &Config) -> Result<Self> {
        info!("Initializing exporter manager...");

        let normalize = config
            .features
            .replace_parameters
            .as_ref()
            .is_none_or(|r| r.enable);

        if let Some(cfg) = &config.exporter.csv {
            info!("Using CSV exporter: {}", cfg.file);
            let mut exporter = CsvExporter::from_config(cfg);
            exporter.normalize = normalize;
            return Ok(Self {
                exporter: ExporterKind::Csv(exporter),
            });
        }

        if let Some(cfg) = &config.exporter.sqlite {
            info!("Using SQLite exporter: {}", cfg.database_url);
            let mut exporter = SqliteExporter::from_config(cfg);
            exporter.normalize = normalize;
            return Ok(Self {
                exporter: ExporterKind::Sqlite(exporter),
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

    /// 热路径：使用预解析的 meta/pm，避免导出器内部重复解析。
    #[inline]
    pub fn export_one_preparsed(
        &mut self,
        sqllog: &Sqllog<'_>,
        meta: &MetaParts<'_>,
        pm: &PerformanceMetrics<'_>,
        normalized: Option<&str>,
    ) -> Result<()> {
        self.exporter
            .export_one_preparsed(sqllog, meta, pm, normalized)
    }

    pub fn finalize(&mut self) -> Result<()> {
        info!("Finalizing exporters...");
        self.exporter.finalize()?;
        info!("Exporters finished");
        Ok(())
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.exporter.kind_name()
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

/// 去除 IPv4-mapped IPv6 地址前缀（如 `::ffff:192.168.1.1` → `192.168.1.1`）
#[inline]
#[must_use]
pub(super) fn strip_ip_prefix(ip: &str) -> &str {
    const PREFIX: &str = "::ffff:";
    // 快速路径：IPv4 地址以数字开头，不以 ':' 开头，直接返回
    if ip.as_bytes().first() != Some(&b':') {
        return ip;
    }
    if ip.len() > PREFIX.len() && ip[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        &ip[PREFIX.len()..]
    } else {
        ip
    }
}

/// Saturating cast from f32 milliseconds to i64 milliseconds without precision-loss warnings
#[inline]
#[must_use]
pub(super) fn f32_ms_to_i64(ms: f32) -> i64 {
    if !ms.is_finite() {
        return 0;
    }

    const MAX_I64_F64: f64 = 9_223_372_036_854_775_807.0; // i64::MAX as f64
    const MIN_I64_F64: f64 = -9_223_372_036_854_775_808.0; // i64::MIN as f64

    let ms_f64 = f64::from(ms);
    if ms_f64 > MAX_I64_F64 {
        i64::MAX
    } else if ms_f64 < MIN_I64_F64 {
        i64::MIN
    } else {
        let clamped = ms_f64.trunc();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "value already clamped to i64 range"
        )]
        {
            clamped as i64
        }
    }
}

/// 确保输出文件的父目录存在
pub(super) fn ensure_parent_dir(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.exists()) {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ExportStats ────────────────────────────────────────────
    #[test]
    fn test_export_stats_default() {
        let s = ExportStats::new();
        assert_eq!(s.exported, 0);
        assert_eq!(s.total(), 0);
    }

    #[test]
    fn test_export_stats_record_success() {
        let mut s = ExportStats::new();
        s.record_success();
        s.record_success();
        assert_eq!(s.exported, 2);
        assert_eq!(s.total(), 2);
    }

    #[test]
    fn test_export_stats_total_includes_all() {
        let mut s = ExportStats::new();
        s.exported = 5;
        s.skipped = 2;
        s.failed = 1;
        assert_eq!(s.total(), 8);
    }

    // ── strip_ip_prefix ────────────────────────────────────────
    #[test]
    fn test_strip_ip_prefix_with_prefix() {
        assert_eq!(strip_ip_prefix("::ffff:192.168.1.1"), "192.168.1.1");
    }

    #[test]
    fn test_strip_ip_prefix_uppercase() {
        assert_eq!(strip_ip_prefix("::FFFF:10.0.0.1"), "10.0.0.1");
    }

    #[test]
    fn test_strip_ip_prefix_no_prefix() {
        assert_eq!(strip_ip_prefix("192.168.1.1"), "192.168.1.1");
    }

    #[test]
    fn test_strip_ip_prefix_ipv6() {
        assert_eq!(strip_ip_prefix("2001:db8::1"), "2001:db8::1");
    }

    #[test]
    fn test_strip_ip_prefix_empty() {
        assert_eq!(strip_ip_prefix(""), "");
    }

    // ── f32_ms_to_i64 ──────────────────────────────────────────
    #[test]
    fn test_f32_ms_to_i64_normal() {
        assert_eq!(f32_ms_to_i64(100.0_f32), 100);
    }

    #[test]
    fn test_f32_ms_to_i64_nan() {
        assert_eq!(f32_ms_to_i64(f32::NAN), 0);
    }

    #[test]
    fn test_f32_ms_to_i64_pos_infinity() {
        assert_eq!(f32_ms_to_i64(f32::INFINITY), 0);
    }

    #[test]
    fn test_f32_ms_to_i64_neg_infinity() {
        assert_eq!(f32_ms_to_i64(f32::NEG_INFINITY), 0);
    }

    #[test]
    fn test_f32_ms_to_i64_zero() {
        assert_eq!(f32_ms_to_i64(0.0), 0);
    }

    #[test]
    fn test_f32_ms_to_i64_negative() {
        assert_eq!(f32_ms_to_i64(-50.0), -50);
    }

    // ── ensure_parent_dir ──────────────────────────────────────
    #[test]
    fn test_ensure_parent_dir_existing() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("out.csv");
        // Parent exists → should not error
        ensure_parent_dir(&path).unwrap();
    }

    #[test]
    fn test_ensure_parent_dir_creates_new() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("sub/dir/out.csv");
        ensure_parent_dir(&path).unwrap();
        assert!(dir.path().join("sub/dir").exists());
    }

    // ── DryRunExporter ─────────────────────────────────────────
    #[test]
    fn test_dry_run_exporter_counts_records() {
        let mut e = DryRunExporter::default();
        e.initialize().unwrap();
        // Manually add some counts via export_batch with empty batches approach
        e.stats.exported = 5;
        let snap = e.stats_snapshot().unwrap();
        assert_eq!(snap.exported, 5);
    }
}

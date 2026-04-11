use crate::config::Config;
use crate::error::{ConfigError, Error, Result};
use dm_database_parser_sqllog::Sqllog;
use log::info;

pub mod csv;
pub mod sqlite;
pub use csv::CsvExporter;
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

    /// 批量导出，同时传入每条记录对应的 `normalized_sql`。
    /// 默认实现忽略 normalized 参数，直接调用 `export_batch`。
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
        Some(self.stats)
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
                exporter: Box::new(exporter),
            });
        }

        if let Some(cfg) = &config.exporter.sqlite {
            info!("Using SQLite exporter: {}", cfg.database_url);
            let mut exporter = SqliteExporter::from_config(cfg);
            exporter.normalize = normalize;
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

    /// 批量导出，同时传入每条记录的 `normalized_sql`
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

/// 去除 IPv4-mapped IPv6 地址前缀（如 `::ffff:192.168.1.1` → `192.168.1.1`）
#[must_use]
pub(super) fn strip_ip_prefix(ip: &str) -> &str {
    const PREFIX: &str = "::ffff:";
    if ip.len() > PREFIX.len() && ip[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        &ip[PREFIX.len()..]
    } else {
        ip
    }
}

/// Saturating cast from f32 milliseconds to i64 milliseconds without precision-loss warnings
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
    fn test_export_stats_record_success_batch() {
        let mut s = ExportStats::new();
        s.record_success_batch(10);
        assert_eq!(s.exported, 10);
        assert_eq!(s.total(), 10);
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

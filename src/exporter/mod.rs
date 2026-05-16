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

    /// 将 SQL 模板聚合统计写入导出目标。
    /// 默认实现为 no-op，向后兼容现有 exporter。
    // Plan 04 将在 run.rs 接入此方法；骨架阶段暂未调用。
    #[allow(dead_code)]
    fn write_template_stats(
        &mut self,
        stats: &[crate::features::TemplateStats],
        final_path: Option<&std::path::Path>,
    ) -> Result<()> {
        let _ = (stats, final_path);
        Ok(())
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

    /// 当前 active exporter 是否应包含性能指标列（仅 CSV 路径有意义）。
    /// 用于 `cli/run.rs` 热循环判断是否需要调用 `record.parse_performance_metrics()`。
    pub fn csv_include_performance_metrics(&self) -> bool {
        match self {
            Self::Csv(exporter) => exporter.include_performance_metrics,
            // SQLite/DryRun 永远需要完整 pm（schema 固定）
            _ => true,
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

    #[inline]
    #[allow(dead_code)]
    fn write_template_stats(
        &mut self,
        stats: &[crate::features::TemplateStats],
        final_path: Option<&std::path::Path>,
    ) -> Result<()> {
        match self {
            Self::Csv(e) => e.write_template_stats(stats, final_path),
            Self::Sqlite(e) => e.write_template_stats(stats, final_path),
            Self::DryRun(e) => e.write_template_stats(stats, final_path),
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

    fn write_template_stats(
        &mut self,
        stats: &[crate::features::TemplateStats],
        _final_path: Option<&std::path::Path>,
    ) -> Result<()> {
        info!(
            "Dry-run: would write {} template stats (no file written)",
            stats.len()
        );
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

        let field_mask = config.features.field_mask();
        let ordered_indices = config.features.ordered_field_indices();

        if let Some(cfg) = &config.exporter.csv {
            info!("Using CSV exporter: {}", cfg.file);
            let mut exporter = CsvExporter::from_config(cfg);
            exporter.normalize = normalize;
            exporter.field_mask = field_mask;
            exporter.ordered_indices.clone_from(&ordered_indices);
            return Ok(Self {
                exporter: ExporterKind::Csv(exporter),
            });
        }

        if let Some(cfg) = &config.exporter.sqlite {
            info!("Using SQLite exporter: {}", cfg.database_url);
            let mut exporter = SqliteExporter::from_config(cfg);
            exporter.normalize = normalize;
            exporter.field_mask = field_mask;
            exporter.ordered_indices = ordered_indices;
            return Ok(Self {
                exporter: ExporterKind::Sqlite(exporter),
            });
        }

        Err(Error::Config(ConfigError::NoExporters))
    }

    /// 返回当前 active exporter 是否应包含性能指标列。
    /// CSV 路径根据配置返回；其他路径固定返回 true。
    pub fn csv_include_performance_metrics(&self) -> bool {
        self.exporter.csv_include_performance_metrics()
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

    // Plan 04 将在 run.rs 接入此方法；骨架阶段暂未调用。
    #[allow(dead_code)]
    pub fn write_template_stats(
        &mut self,
        stats: &[crate::features::TemplateStats],
        final_path: Option<&std::path::Path>,
    ) -> Result<()> {
        self.exporter.write_template_stats(stats, final_path)
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

    // ── ExporterManager constructors ───────────────────────────
    #[test]
    fn test_from_csv_constructor() {
        let exporter = CsvExporter::new(std::path::PathBuf::from("/tmp/test.csv"));
        let manager = ExporterManager::from_csv(exporter);
        assert_eq!(manager.name(), "CSV");
    }

    #[test]
    fn test_dry_run_constructor() {
        let manager = ExporterManager::dry_run();
        assert_eq!(manager.name(), "dry-run");
    }

    #[test]
    fn test_from_config_sqlite_path() {
        use crate::config::SqliteExporter as SqliteExporterCfg;
        use crate::config::{Config, ExporterConfig, SqllogConfig};
        let cfg = Config {
            exporter: ExporterConfig {
                csv: None,
                sqlite: Some(SqliteExporterCfg {
                    database_url: "/tmp/test_mod.db".to_string(),
                    table_name: "records".to_string(),
                    overwrite: true,
                    append: false,
                    batch_size: 10_000,
                }),
            },
            sqllog: SqllogConfig {
                path: "sqllogs".to_string(),
            },
            ..Default::default()
        };
        let manager = ExporterManager::from_config(&cfg).unwrap();
        assert_eq!(manager.name(), "SQLite");
    }

    #[test]
    fn test_from_config_no_exporters_error() {
        use crate::config::{Config, ExporterConfig, SqllogConfig};
        let cfg = Config {
            exporter: ExporterConfig {
                csv: None,
                sqlite: None,
            },
            sqllog: SqllogConfig {
                path: "sqllogs".to_string(),
            },
            ..Default::default()
        };
        let result = ExporterManager::from_config(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_log_stats_with_flush_operations() {
        let mut stats = ExportStats::new();
        stats.exported = 10;
        stats.flush_operations = 2;
        stats.last_flush_size = 5;
        // log_stats reads from the exporter kind — use DryRunExporter and manually check
        let e = DryRunExporter { stats };
        let snap = e.stats_snapshot().unwrap();
        assert_eq!(snap.flush_operations, 2);
        assert_eq!(snap.last_flush_size, 5);
    }

    #[test]
    fn test_exporter_manager_log_stats_no_panic() {
        let manager = ExporterManager::dry_run();
        // Just verify it doesn't panic
        manager.log_stats();
    }

    #[test]
    fn test_exporter_manager_debug_format() {
        let manager = ExporterManager::dry_run();
        let s = format!("{manager:?}");
        assert!(s.contains("ExporterManager"));
    }

    #[test]
    fn test_dry_run_export_via_trait() {
        use dm_database_parser_sqllog::LogParser;
        let dir = tempfile::TempDir::new().unwrap();
        let log = dir.path().join("t.log");
        std::fs::write(&log, "2025-01-15 10:30:28.001 (EP[0] sess:0x0001 user:U trxid:1 stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT 1. EXECTIME: 1(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n").unwrap();
        let parser = LogParser::from_path(log.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().flatten().collect();

        let mut e = DryRunExporter::default();
        e.initialize().unwrap();
        for r in &records {
            e.export(r).unwrap();
        }
        e.finalize().unwrap();
        let snap = e.stats_snapshot().unwrap();
        assert_eq!(snap.exported, records.len());
    }

    #[test]
    fn test_f32_ms_to_i64_large_positive() {
        // Value larger than i64::MAX should return i64::MAX
        let result = f32_ms_to_i64(f32::MAX);
        assert_eq!(result, i64::MAX);
    }

    #[test]
    fn test_strip_ip_prefix_colon_non_ffff() {
        // Starts with ':' but not the exact ffff prefix
        assert_eq!(strip_ip_prefix("::1"), "::1");
    }

    // ── write_template_stats ───────────────────────────────────

    /// 辅助：构造一个最小 `TemplateStats` 实例
    fn make_template_stats(key: &str) -> crate::features::TemplateStats {
        crate::features::TemplateStats {
            template_key: key.to_string(),
            count: 1,
            avg_us: 100,
            min_us: 10,
            max_us: 200,
            p50_us: 90,
            p95_us: 180,
            p99_us: 195,
            first_seen: "2025-01-01 00:00:00".to_string(),
            last_seen: "2025-01-01 01:00:00".to_string(),
        }
    }

    /// Test 1: 自定义 mock exporter 不覆盖 `write_template_stats`，默认 no-op 返回 `Ok(())`
    #[test]
    fn test_default_write_template_stats_noop() {
        #[derive(Debug, Default)]
        struct MockExporter;

        impl Exporter for MockExporter {
            fn initialize(&mut self) -> Result<()> {
                Ok(())
            }
            fn export(&mut self, _: &dm_database_parser_sqllog::Sqllog<'_>) -> Result<()> {
                Ok(())
            }
            fn finalize(&mut self) -> Result<()> {
                Ok(())
            }
            // write_template_stats 未覆盖 → 使用 trait 默认 no-op
        }

        let mut mock = MockExporter;
        let stats = vec![make_template_stats("SELECT ?")];
        let result = mock.write_template_stats(&stats, None);
        assert!(result.is_ok());
    }

    /// Test 2: `DryRunExporter` 覆盖为 no-op，不创建任何文件
    #[test]
    fn test_dry_run_write_template_stats_noop() {
        let mut e = DryRunExporter::default();
        e.initialize().unwrap();
        let before = e.stats_snapshot().unwrap().exported;

        let stats = vec![
            make_template_stats("SELECT ?"),
            make_template_stats("INSERT ?"),
        ];
        let result = e.write_template_stats(&stats, None);
        assert!(result.is_ok());

        // write_template_stats 不影响 exported 计数
        let after = e.stats_snapshot().unwrap().exported;
        assert_eq!(before, after);
    }

    /// Test 3: `ExporterManager::dry_run()` 委托调用 `write_template_stats` 返回 `Ok(())`
    #[test]
    fn test_exporter_manager_write_template_stats_dry_run() {
        let mut manager = ExporterManager::dry_run();
        let stats = vec![make_template_stats("SELECT ?")];
        let result = manager.write_template_stats(&stats, None);
        assert!(result.is_ok());
    }

    /// Test 4: `ExporterKind` 三个 variant 透传 `write_template_stats` 均不 panic
    #[test]
    fn test_exporter_kind_dispatch_write_template_stats() {
        let stats: Vec<crate::features::TemplateStats> = vec![];

        // DryRun variant
        let mut dry_run = ExporterKind::DryRun(DryRunExporter::default());
        assert!(dry_run.write_template_stats(&stats, None).is_ok());

        // CSV variant — 默认实现（Plan 02 尚未实现具体覆盖）
        let csv = CsvExporter::new(std::path::PathBuf::from("/tmp/test_dispatch.csv"));
        let mut csv_kind = ExporterKind::Csv(csv);
        assert!(csv_kind.write_template_stats(&stats, None).is_ok());

        // SQLite variant — 默认实现（Plan 02 尚未实现具体覆盖）
        use crate::config::SqliteExporter as SqliteExporterCfg;
        let sqlite_cfg = SqliteExporterCfg {
            database_url: "/tmp/test_dispatch.db".to_string(),
            table_name: "records".to_string(),
            overwrite: true,
            append: false,
            batch_size: 10_000,
        };
        let sqlite = SqliteExporter::from_config(&sqlite_cfg);
        let mut sqlite_kind = ExporterKind::Sqlite(sqlite);
        assert!(sqlite_kind.write_template_stats(&stats, None).is_ok());
    }
}

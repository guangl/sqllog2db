//! 针对 exporter 模块的深入覆盖测试
#[cfg(test)]
mod exporter_deep_tests {
    use dm_database_sqllog2db::exporter::ExportStats;
    use std::fs;
    use std::path::PathBuf;

    #[allow(dead_code)]
    fn setup_test_dir(name: &str) -> PathBuf {
        let test_dir = PathBuf::from("target/test_exporter_deep").join(name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).expect("Failed to create test dir");
        test_dir
    }

    #[test]
    fn test_export_stats_creation() {
        let stats = ExportStats::default();
        assert_eq!(stats.exported, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_export_stats_new() {
        let stats = ExportStats::new();
        assert_eq!(stats.exported, 0);
    }

    #[test]
    fn test_export_stats_record_success() {
        let mut stats = ExportStats::new();
        stats.record_success();
        assert_eq!(stats.exported, 1);
    }

    #[test]
    fn test_export_stats_debug() {
        let stats = ExportStats::default();
        let debug = format!("{stats:?}");
        assert!(debug.contains("ExportStats"));
    }

    #[test]
    fn test_export_stats_clone() {
        let mut stats1 = ExportStats::new();
        stats1.exported = 100;
        stats1.skipped = 50;

        let stats2 = stats1.clone();
        assert_eq!(stats1.exported, stats2.exported);
        assert_eq!(stats1.skipped, stats2.skipped);
    }

    #[test]
    fn test_export_stats_total() {
        let mut stats = ExportStats::new();
        stats.exported = 100;
        stats.skipped = 20;
        stats.failed = 5;

        assert_eq!(stats.total(), 125);
    }

    #[test]
    fn test_export_stats_multiple_successes() {
        let mut stats = ExportStats::new();
        for _ in 0..10 {
            stats.record_success();
        }
        assert_eq!(stats.exported, 10);
    }

    #[test]
    fn test_export_stats_skipped_records() {
        let mut stats = ExportStats::new();
        stats.skipped = 100;

        assert_eq!(stats.skipped, 100);
        assert_eq!(stats.total(), 100);
    }

    #[test]
    fn test_export_stats_failed_records() {
        let mut stats = ExportStats::new();
        stats.failed = 5;

        assert_eq!(stats.failed, 5);
    }

    #[test]
    fn test_export_stats_flush_operations() {
        let mut stats = ExportStats::new();
        stats.flush_operations = 10;

        assert_eq!(stats.flush_operations, 10);
    }

    #[test]
    fn test_export_stats_last_flush_size() {
        let mut stats = ExportStats::new();
        stats.last_flush_size = 1000;

        assert_eq!(stats.last_flush_size, 1000);
    }

    #[test]
    fn test_export_stats_all_fields() {
        let mut stats = ExportStats::new();
        stats.exported = 900;
        stats.skipped = 50;
        stats.failed = 50;
        stats.flush_operations = 9;
        stats.last_flush_size = 100;

        assert_eq!(stats.total(), 1000);
        assert_eq!(stats.flush_operations, 9);
    }

    #[test]
    fn test_export_stats_zero_total() {
        let stats = ExportStats::new();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn test_export_stats_large_numbers() {
        let mut stats = ExportStats::new();
        stats.exported = 10_000_000;
        stats.skipped = 1_000_000;

        assert_eq!(stats.total(), 11_000_000);
    }

    #[test]
    fn test_export_stats_throughput_calculation() {
        let mut stats = ExportStats::new();
        stats.exported = 1000;
        stats.flush_operations = 10;

        let avg_per_flush = stats.exported / (stats.flush_operations + 1);
        assert!(avg_per_flush > 0);
    }
}

use crate::config::Config;
use log::info;

pub fn handle_validate(cfg: &Config) {
    info!("SQL日志输入目录: {}", cfg.sqllog.directory);
    info!("日志级别: {}", cfg.logging.level);
    info!("日志文件: {}", cfg.logging.file);
    info!("日志保留: {} 天", cfg.logging.retention_days);
    info!("错误日志: {}", cfg.error.file);

    #[cfg(feature = "filters")]
    if let Some(f) = &cfg.features.filters {
        info!(
            "Feature flags - filters: {}",
            if f.enable {
                "启用"
            } else {
                "配置但未明确启用"
            }
        );
    }

    #[cfg(feature = "csv")]
    if let Some(csv) = &cfg.exporter.csv {
        info!(
            "CSV export: {} (overwrite: {})",
            csv.file,
            if csv.overwrite { "yes" } else { "no" }
        );
    }
}

use crate::config::Config;
use log::info;

pub fn handle_validate(cfg: &Config) {
    info!("SQL日志输入路径: {}", cfg.sqllog.path);
    info!("日志级别: {}", cfg.logging.level);
    info!("日志文件: {}", cfg.logging.file);
    info!("日志保留: {} 天", cfg.logging.retention_days);

    match &cfg.features.replace_parameters {
        Some(rp) => info!(
            "features.replace_parameters: enable={}, placeholders={:?}",
            rp.enable, rp.placeholders
        ),
        None => info!("features.replace_parameters: 未配置（默认启用，自动检测占位符）"),
    }
    match &cfg.features.filters {
        Some(f) => {
            info!(
                "features.filters: {}",
                if f.enable {
                    "启用"
                } else {
                    "配置但未明确启用"
                }
            );
            if let Some(start) = &f.meta.start_ts {
                info!("  start_ts = {start}");
            }
            if let Some(end) = &f.meta.end_ts {
                info!("  end_ts = {end}");
            }
            if let Some(ids) = &f.meta.trxids {
                info!("  trxids = {} 条", ids.len());
            }
            if let Some(users) = &f.meta.usernames {
                info!("  usernames = {users:?}");
            }
            if let Some(ips) = &f.meta.client_ips {
                info!("  client_ips = {ips:?}");
            }
            if let Some(ids) = &f.indicators.exec_ids {
                info!("  exec_ids = {} 条", ids.len());
            }
            if let Some(ms) = f.indicators.min_runtime_ms {
                info!("  min_runtime_ms = {ms}");
            }
            if let Some(rows) = f.indicators.min_row_count {
                info!("  min_row_count = {rows}");
            }
            if f.sql.has_filters() {
                info!(
                    "  sql.include_patterns = {} 条, exclude_patterns = {} 条",
                    f.sql.include_patterns.as_ref().map_or(0, Vec::len),
                    f.sql.exclude_patterns.as_ref().map_or(0, Vec::len),
                );
            }
        }
        None => info!("features.filters: 未配置"),
    }

    if let Some(csv) = &cfg.exporter.csv {
        info!(
            "CSV export: {} (overwrite: {})",
            csv.file,
            if csv.overwrite { "yes" } else { "no" }
        );
    }
    if let Some(sqlite) = &cfg.exporter.sqlite {
        info!(
            "SQLite export: {} / {} (overwrite: {})",
            sqlite.database_url,
            sqlite.table_name,
            if sqlite.overwrite { "yes" } else { "no" }
        );
    }
}

use log::info;

use crate::config::Config;
use crate::error::Result;

/// 验证配置文件
pub fn handle_validate(cfg: &Config) -> Result<()> {
    info!("配置验证已在 main 中完成");

    info!("SQL日志输入目录: {}", cfg.sqllog.directory());
    info!("日志级别: {}", cfg.logging.level());
    info!("日志文件: {}", cfg.logging.file());
    info!("日志保留: {} 天", cfg.logging.retention_days());
    info!("错误日志: {}", cfg.error.file());

    info!(
        "Feature flags - replace SQL params: {}",
        if cfg.features.should_replace_sql_parameters() {
            "启用"
        } else {
            "禁用"
        },
    );

    if let Some(rp) = &cfg.features.replace_parameters
        && let Some(symbols) = &rp.symbols
    {
        info!("SQL参数占位符样式: {symbols:?}");
    }

    // 导出配置（只支持单个导出器）
    if let Some(csv) = &cfg.exporter.csv {
        info!(
            "CSV export: {} (overwrite: {})",
            csv.file,
            if csv.overwrite { "yes" } else { "no" }
        );
    }

    Ok(())
}

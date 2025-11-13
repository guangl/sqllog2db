use tracing::info;

use crate::config::Config;
use crate::error::Result;

/// 验证配置文件
pub fn handle_validate(cfg: &Config) -> Result<()> {
    info!("配置验证已在 main 中完成");

    info!("SQL日志输入目录: {}", cfg.sqllog.directory());
    info!("批量大小: {}", cfg.sqllog.batch_size());
    info!("日志级别: {}", cfg.logging.level());
    info!("日志文件: {}", cfg.logging.file());
    info!("日志保留: {} 天", cfg.logging.retention_days());
    info!("错误日志: {}", cfg.error.file());

    info!(
        "功能特性 - 替换 SQL 参数: {}, 生成散列表: {}",
        if cfg.features.should_replace_sql_parameters() {
            "启用"
        } else {
            "禁用"
        },
        if cfg.features.should_scatter() {
            "启用"
        } else {
            "禁用"
        }
    );

    // 导出配置（只支持单个导出器）
    if let Some(csv) = &cfg.exporter.csv {
        info!(
            "CSV导出: {} (覆盖: {})",
            csv.file,
            if csv.overwrite { "是" } else { "否" }
        );
    }
    #[cfg(feature = "sqlite")]
    if cfg.exporter.csv.is_none() {
        if let Some(sqlite) = &cfg.exporter.sqlite {
            info!(
                "SQLite导出: {} -> {} (覆盖: {})",
                sqlite.file,
                sqlite.table_name,
                if sqlite.overwrite { "是" } else { "否" }
            );
        } else {
            info!("导出器: 未配置");
        }
    }
    #[cfg(not(feature = "sqlite"))]
    if cfg.exporter.csv.is_none() {
        info!("导出器: 未配置");
    }

    Ok(())
}

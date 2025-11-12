use tracing::info;

use crate::config::Config;
use crate::error::Result;

/// 验证配置文件
pub fn handle_validate(cfg: &Config) -> Result<()> {
    info!("配置验证已在 main 中完成");

    info!("SQL日志路径: {}", cfg.sqllog.path());
    info!("批量大小: {}", cfg.sqllog.batch_size());
    info!("日志级别: {}", cfg.logging.level());
    info!("日志文件: {}", cfg.logging.path());
    info!("日志保留: {} 天", cfg.logging.retention_days());
    info!("错误日志: {}", cfg.error.path());

    info!(
        "功能特性 - 替换SQL参数: {}, 分散导出: {}",
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
    if let Some(db) = &cfg.exporter.database {
        info!(
            "数据库导出: {} ({}:{} -> {} 覆盖: {})",
            db.database_type.as_str(),
            if db.host.is_empty() { "N/A" } else { &db.host },
            db.port,
            db.table_name,
            if db.overwrite { "是" } else { "否" }
        );
    } else if let Some(csv) = &cfg.exporter.csv {
        info!(
            "CSV导出: {} (覆盖: {})",
            csv.path(),
            if csv.overwrite { "是" } else { "否" }
        );
    } else if let Some(jsonl) = &cfg.exporter.jsonl {
        info!(
            "JSONL导出: {} (覆盖: {})",
            jsonl.path(),
            if jsonl.overwrite { "是" } else { "否" }
        );
    } else {
        info!("导出器: 未配置");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 注意:测试已改为接收 Config 对象,配置验证在 main.rs 中进行

    #[test]
    fn test_validate_nonexistent_file() {
        // 测试文件不存在的情况现在在 main.rs 中处理
        // 这里测试默认配置是否有效
        let cfg = Config::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_valid_config() {
        let toml_str = r#"
[sqllog]
path = "sqllog"
batch_size = 10000

[error]
path = "errors.jsonl"

[logging]
path = "logs/app.log"
level = "info"

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "output.csv"
overwrite = true
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.validate().is_ok());
        let result = handle_validate(&cfg);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_auto_threading() {
        let toml_str = r#"
[sqllog]
path = "sqllog"
batch_size = 10000

[error]
path = "errors.jsonl"

[logging]
path = "logs/app.log"
level = "debug"

[features]
replace_sql_parameters = true
scatter = true

[exporter.jsonl]
path = "output.jsonl"
overwrite = true
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.sqllog.batch_size(), 10000);
        let result = handle_validate(&cfg);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_with_multiple_exporters() {
        let toml_str = r#"
[sqllog]
path = "sqllog"
batch_size = 0

[error]
path = "errors.jsonl"

[logging]
path = "logs/app.log"
level = "warn"

[features]
replace_sql_parameters = false
scatter = false

[exporter.database]
database_type = "dm"
host = "localhost"
port = 5236
username = "admin"
password = "password"
overwrite = true
table_name = "test_table"

"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.validate().is_ok());
        assert!(cfg.exporter.database().is_some());
        let result = handle_validate(&cfg);
        assert!(result.is_ok());
    }
}

use crate::error::{Error, FileError, Result};
use log::{debug, error, info, warn};
use std::fs;
use std::path::Path;

/// 生成默认配置文件
pub fn handle_init(output_path: &str, force: bool) -> Result<()> {
    let path = Path::new(output_path);

    info!("Preparing to generate configuration file: {output_path}");

    // 检查文件是否已存在
    if path.exists() && !force {
        error!("Configuration file already exists: {output_path}");
        info!("Tip: use --force to overwrite");
        return Err(Error::File(FileError::AlreadyExists {
            path: path.to_path_buf(),
        }));
    }

    if path.exists() && force {
        warn!("Will overwrite existing configuration file");
    }

    // 生成默认配置内容
    debug!("Generating default configuration content...");
    let default_config = r#"# SQL 日志导出工具默认配置文件 (请根据需要修改)

[sqllog]
# SQL 日志目录或文件路径
directory = "sqllogs"

[error]
# 解析错误日志输出路径（纯文本行: file | error | raw | line）
file = "export/errors.log"

[logging]
# 应用日志输出目录或文件路径 (当前版本要求为"文件路径"，例如 logs/sqllog2db.log)
# 如果仅设置为目录（如 "logs"），请确保后续代码逻辑能够自动生成文件；否则请填写完整文件路径
file = "logs/sqllog2db.log"
# 日志级别: trace | debug | info | warn | error
level = "info"
# 日志保留天数 (1-365) - 用于滚动文件最大保留数量
retention_days = 7

[features.replace_parameters]
enable = false
symbols = ["?", ":name", "$1"] # 可选参数占位符样式列表

# ===================== 导出器配置 =====================
# 只能配置一个导出器
# 同时配置多个时，按优先级使用：csv > parquet > jsonl > sqlite > duckdb > postgres > dm

# 方案 1: csv 导出（默认）
[exporter.csv]
file = "outputs/sqllog.csv"
overwrite = true
append = false

# 方案 2: Parquet 导出（使用时注释掉上面的导出器,启用下面的 Parquet）
# [exporter.parquet]
# file = "export/sqllog2db.parquet"
# overwrite = true
# row_group_size = 100000           # 每个 row group 的行数 (默认)
# use_dictionary = true             # 是否启用字典编码

# 方案 3: JSONL 导出（JSON Lines 格式，每行一个 JSON 对象）
# [exporter.jsonl]
# file = "export/sqllog2db.jsonl"
# overwrite = true
# append = false

# 方案 4: SQLite 数据库导出
# [exporter.sqlite]
# database_url = "export/sqllog2db.db"
# table_name = "sqllog_records"
# overwrite = true
# append = false

# 方案 5: DuckDB 数据库导出（分析型数据库，高性能）
# [exporter.duckdb]
# database_url = "export/sqllog2db.duckdb"
# table_name = "sqllog_records"
# overwrite = true
# append = false

# 方案 6: PostgreSQL 数据库导出
# [exporter.postgres]
# host = "localhost"
# port = 5432
# username = "postgres"
# password = "postgres"
# database = "sqllog"
# schema = "public"
# table_name = "sqllog_records"
# overwrite = true
# append = false

# 方案 7: DM 数据库导出（使用 dmfldr 命令行工具）
# [exporter.dm]
# userid = "SYSDBA/SYSDBA@localhost:5236"
# table_name = "sqllog_records"
# control_file = "export/sqllog.ctl"
# log_dir = "export/log"
# overwrite = true
# charset = "UTF-8"

"#;

    // 创建目录（如果需要）
    if let Some(parent) = path.parent().filter(|p| !p.exists()) {
        info!("Creating directory: {}", parent.display());
        fs::create_dir_all(parent).map_err(|e| {
            crate::error::Error::File(crate::error::FileError::CreateDirectoryFailed {
                path: parent.to_path_buf(),
                reason: e.to_string(),
            })
        })?;
    }

    // 写入配置文件
    debug!("Writing configuration file...");
    fs::write(path, default_config).map_err(|e| {
        Error::File(FileError::WriteFailed {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })
    })?;

    if force && path.exists() {
        info!("Configuration file overwritten: {output_path}");
    } else {
        info!("Configuration file generated: {output_path}");
    }

    info!("Next steps:");
    info!("  1. Edit configuration file: {output_path}");
    info!("  2. Validate configuration: sqllog2db validate -c {output_path}");
    info!("  3. Run export: sqllog2db run -c {output_path}");

    Ok(())
}

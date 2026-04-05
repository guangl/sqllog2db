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

[logging]
# 应用日志输出目录或文件路径 (当前版本要求为"文件路径"，例如 logs/sqllog2db.log)
# 如果仅设置为目录（如 "logs"），请确保后续代码逻辑能够自动生成文件；否则请填写完整文件路径
file = "logs/sqllog2db.log"
# 日志级别: trace | debug | info | warn | error
level = "info"
# 日志保留天数 (1-365) - 用于滚动文件最大保留数量
retention_days = 7

[features.replace_parameters]
# 是否在导出结果中写入 normalized_sql 列
# 对 INS/DEL/UPD/ORA 类型的记录，将 PARAMS 参数值填入 SQL 的 ? 占位符
enable = true

[features.filters]
# 是否启用过滤器
enable = false

# --- 元数据过滤器 (Record-level: 满足任一条件即保留该条记录) ---
# 过滤指定的事务 ID
# trxids = ["257809109", "257809110"]
# 过滤指定的客户端 IP (支持模糊匹配)
# client_ips = ["127.0.0.1", "192.168"]
# 过滤指定的用户名 (支持模糊匹配)
# usernames = ["SYSDBA"]
# 过滤时间范围 (格式：2023-01-01 00:00:00)
# start_ts = "2023-01-01 00:00:00"
# end_ts = "2023-01-01 23:59:59"
# 过滤指定的会话 ID (支持模糊匹配)
# sess_ids = ["0x7f41435437a8"]
# 过滤指定的线程 ID (支持模糊匹配)
# thrd_ids = ["2188515"]
# 过滤指定的语句类型 (支持模糊匹配)
# statements = ["INS", "UPD", "DEL"]
# 过滤指定的应用名称 (支持模糊匹配)
# appnames = ["DMSQL"]

# --- 指标过滤器 (Transaction-level: 满足条件则保留包含该语句的整个事务 - 需要预扫描) ---
[features.filters.indicators]
# 过滤指定的执行 ID (保留整个事务)
# exec_ids = [257809109, 257809110]
# 过滤最小执行时长 (毫秒)
# min_runtime_ms = 1000
# 过滤最小影响行数
# min_row_count = 100

# --- SQL 过滤器 (Transaction-level: 满足模式则保留整个事务 - 需要预扫描) ---
[features.filters.sql]
# 包含模式列表 (SQL 包含任一模式则匹配)
# include_patterns = ["FROM USER_TABLES", "DELETE FROM"]
# 排除模式列表 (SQL 包含任一模式则剔除)
# exclude_patterns = ["SELECT 1", "DUAL"]

# ===================== 断点续传 =====================
# 使用 --resume 标志时，sqllog2db 会跳过已成功处理的文件（通过文件大小和修改时间判断）。
# [resume]
# state_file = ".sqllog2db_state.toml"

# ===================== 导出器配置 =====================
# 只能配置一个导出器
# 同时配置多个时，按优先级使用：csv > sqlite

# 方案 1: csv 导出（默认）
[exporter.csv]
file = "outputs/sqllog.csv"
overwrite = true
append = false

# 方案 2: SQLite 数据库导出
# [exporter.sqlite]
# database_url = "export/sqllog2db.db"
# table_name = "sqllog_records"
# overwrite = true
# append = false

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

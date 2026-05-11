use crate::error::{Error, FileError, Result};
use crate::lang::Lang;
use log::{debug, error, info, warn};
use std::fs;
use std::path::Path;

/// 生成默认配置文件
pub fn handle_init(output_path: &str, force: bool, lang: Lang) -> Result<()> {
    let path = Path::new(output_path);

    info!("Preparing to generate configuration file: {output_path}");

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

    debug!("Generating default configuration content...");
    let content = match lang {
        Lang::Zh => CONFIG_TEMPLATE_ZH,
        Lang::En => CONFIG_TEMPLATE_EN,
    };

    if let Some(parent) = path.parent().filter(|p| !p.exists()) {
        info!("Creating directory: {}", parent.display());
        fs::create_dir_all(parent).map_err(|e| {
            Error::File(FileError::CreateDirectoryFailed {
                path: parent.to_path_buf(),
                reason: e.to_string(),
            })
        })?;
    }

    debug!("Writing configuration file...");
    fs::write(path, content).map_err(|e| {
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

// ── Templates ────────────────────────────────────────────────────────────────

const CONFIG_TEMPLATE_ZH: &str = r#"# SQL 日志导出工具默认配置文件（请根据需要修改）

[sqllog]
# SQL 日志路径：目录、单文件或 glob 模式（如 "./logs/2025-*.log"）
path = "sqllogs"

[logging]
# 应用日志文件路径
file = "logs/sqllog2db.log"
# 日志级别: trace | debug | info | warn | error
level = "info"
# 日志保留天数 (1-365)
retention_days = 7

[features.replace_parameters]
# 是否在导出结果中写入 normalized_sql 列（默认 true）
# 对 INS/DEL/UPD/ORA 类型的记录，将 PARAMS 参数值填入 SQL 的占位符
enable = true

[features.filters]
# 是否启用过滤器
enable = false

# --- 元数据过滤器（Record-level：满足任一条件即保留该条记录）---
# 过滤指定的事务 ID
# trxids = ["257809109", "257809110"]

# 过滤指定的客户端 IP（支持正则匹配）
# client_ips = ["127.0.0.1", "192\\.168"]
# 排除指定的客户端 IP（OR veto：任一命中则丢弃该记录）
# exclude_client_ips = ["^10\\.0", "^172\\.16"]

# 过滤指定的用户名（支持正则匹配）
# usernames = ["SYSDBA"]
# 排除指定的用户名（OR veto：任一命中则丢弃该记录）
# exclude_usernames = ["guest", "^anon"]

# 过滤时间范围（格式：2023-01-01 00:00:00）
# start_ts = "2023-01-01 00:00:00"
# end_ts   = "2023-01-01 23:59:59"

# 过滤指定的会话 ID（支持正则匹配）
# sess_ids = ["0x7f41435437a8"]
# 排除指定的会话 ID（OR veto：任一命中则丢弃该记录）
# exclude_sess_ids = ["^0x0000"]

# 过滤指定的线程 ID（支持正则匹配）
# thrd_ids = ["2188515"]
# 排除指定的线程 ID（OR veto：任一命中则丢弃该记录）
# exclude_thrd_ids = ["^0$"]

# 过滤指定的语句类型（支持正则匹配）
# statements = ["INS", "UPD", "DEL"]
# 排除指定的语句类型（OR veto：任一命中则丢弃该记录）
# exclude_statements = ["SEL", "SET"]

# 过滤指定的应用名称（支持正则匹配）
# appnames = ["DMSQL"]
# 排除指定的应用名称（OR veto：任一命中则丢弃该记录）
# exclude_appnames = ["monitor", "health"]

# 过滤指定的 tag（支持正则匹配）
# tags = ["\\[SEL\\]"]
# 排除指定的 tag（OR veto：任一命中则丢弃该记录）
# exclude_tags = ["\\[SET\\]", "\\[OTH\\]"]

# --- 指标过滤器（Transaction-level：满足条件则保留包含该语句的整个事务，需要预扫描）---
[features.filters.indicators]
# 过滤指定的执行 ID（保留整个事务）
# exec_ids = [257809109, 257809110]
# 过滤最小执行时长（毫秒）
# min_runtime_ms = 1000
# 过滤最小影响行数
# min_row_count = 100

# --- SQL 过滤器（Transaction-level：满足模式则保留整个事务，需要预扫描）---
[features.filters.sql]
# 包含模式列表（SQL 包含任一模式则匹配）
# include_patterns = ["FROM USER_TABLES", "DELETE FROM"]
# 排除模式列表（SQL 包含任一模式则剔除）
# exclude_patterns = ["SELECT 1", "DUAL"]

# ===================== 断点续传 =====================
# 使用 --resume 标志时，sqllog2db 会跳过已成功处理的文件（通过文件大小和修改时间判断）。
# [resume]
# state_file = ".sqllog2db_state.toml"

# ===================== 导出器配置 =====================
# 只能配置一个导出器，同时配置多个时按优先级使用：csv > sqlite

# 方案 1：CSV 导出（默认）
[exporter.csv]
file = "outputs/sqllog.csv"
overwrite = true
append = false

# 方案 2：SQLite 数据库导出
# [exporter.sqlite]
# database_url = "export/sqllog2db.db"
# table_name = "sqllog_records"
# overwrite = true
# append = false
"#;

const CONFIG_TEMPLATE_EN: &str = r#"# sqllog2db default configuration file (edit as needed)

[sqllog]
# SQL log path: directory, single file, or glob pattern (e.g. "./logs/2025-*.log")
path = "sqllogs"

[logging]
# Application log file path
file = "logs/sqllog2db.log"
# Log level: trace | debug | info | warn | error
level = "info"
# Log retention in days (1-365)
retention_days = 7

[features.replace_parameters]
# Write a normalized_sql column in export output (default: true).
# For INS/DEL/UPD/ORA records, parameter values are substituted into SQL placeholders.
enable = true

[features.filters]
# Enable the filter pipeline
enable = false

# --- Meta filters (record-level: any match retains the record) ---
# Filter by transaction IDs
# trxids = ["257809109", "257809110"]

# Filter by client IPs (regex match)
# client_ips = ["127.0.0.1", "192\\.168"]
# Exclude by client IPs (OR veto: any match drops the record)
# exclude_client_ips = ["^10\\.0", "^172\\.16"]

# Filter by usernames (regex match)
# usernames = ["SYSDBA"]
# Exclude by usernames (OR veto: any match drops the record)
# exclude_usernames = ["guest", "^anon"]

# Filter by time range (format: 2023-01-01 00:00:00)
# start_ts = "2023-01-01 00:00:00"
# end_ts   = "2023-01-01 23:59:59"

# Filter by session IDs (regex match)
# sess_ids = ["0x7f41435437a8"]
# Exclude by session IDs (OR veto: any match drops the record)
# exclude_sess_ids = ["^0x0000"]

# Filter by thread IDs (regex match)
# thrd_ids = ["2188515"]
# Exclude by thread IDs (OR veto: any match drops the record)
# exclude_thrd_ids = ["^0$"]

# Filter by statement types (regex match)
# statements = ["INS", "UPD", "DEL"]
# Exclude by statement types (OR veto: any match drops the record)
# exclude_statements = ["SEL", "SET"]

# Filter by application names (regex match)
# appnames = ["DMSQL"]
# Exclude by application names (OR veto: any match drops the record)
# exclude_appnames = ["monitor", "health"]

# Filter by tags (regex match)
# tags = ["\\[SEL\\]"]
# Exclude by tags (OR veto: any match drops the record)
# exclude_tags = ["\\[SET\\]", "\\[OTH\\]"]

# --- Indicator filters (transaction-level: match retains the whole transaction; requires pre-scan) ---
[features.filters.indicators]
# Filter by execution IDs (retains entire transaction)
# exec_ids = [257809109, 257809110]
# Filter by minimum execution time (ms)
# min_runtime_ms = 1000
# Filter by minimum affected rows
# min_row_count = 100

# --- SQL filters (transaction-level: match retains the whole transaction; requires pre-scan) ---
[features.filters.sql]
# Include patterns (SQL matching any pattern is retained)
# include_patterns = ["FROM USER_TABLES", "DELETE FROM"]
# Exclude patterns (SQL matching any pattern is dropped)
# exclude_patterns = ["SELECT 1", "DUAL"]

# ===================== Resume / Checkpoint =====================
# With --resume, sqllog2db skips files already successfully processed
# (tracked by file size and modification time).
# [resume]
# state_file = ".sqllog2db_state.toml"

# ===================== Exporter Configuration =====================
# Only one exporter can be active at a time. Priority: csv > sqlite

# Option 1: CSV export (default)
[exporter.csv]
file = "outputs/sqllog.csv"
overwrite = true
append = false

# Option 2: SQLite database export
# [exporter.sqlite]
# database_url = "export/sqllog2db.db"
# table_name = "sqllog_records"
# overwrite = true
# append = false
"#;

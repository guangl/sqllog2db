//! 公共常量与复用函数
//! 提供：
//! - 合法日志级别常量 LOG_LEVELS
//! - 数据库表结构与 SQL 语句模板

/// 合法的日志级别（统一来源）
pub const LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];

/// 数据库表列定义（所有数据库导出器共用）
#[allow(dead_code)]
const DB_TABLE_COLUMNS: &str = "
    timestamp TEXT NOT NULL,
    ep INTEGER NOT NULL,
    sess_id TEXT NOT NULL,
    thrd_id TEXT NOT NULL,
    username TEXT,
    trx_id TEXT,
    statement TEXT,
    appname TEXT,
    client_ip TEXT,
    sql TEXT NOT NULL,
    exec_time_ms INTEGER,
    row_count INTEGER,
    exec_id TEXT
";

/// 生成 CREATE TABLE SQL 语句
#[cfg(any(feature = "sqlite", feature = "dm"))]
pub fn create_table_sql(table_name: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {} ({});",
        table_name, DB_TABLE_COLUMNS
    )
}

/// 生成 DROP TABLE SQL 语句
#[cfg(any(feature = "sqlite", feature = "dm"))]
pub fn drop_table_sql(table_name: &str) -> String {
    format!("DROP TABLE IF EXISTS {};", table_name)
}

/// 生成 INSERT SQL 语句
#[cfg(any(feature = "sqlite", feature = "dm"))]
pub fn insert_sql(table_name: &str) -> String {
    format!(
        "INSERT INTO {} (timestamp, ep, sess_id, thrd_id, username, trx_id, statement, \
         appname, client_ip, sql, exec_time_ms, row_count, exec_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
        table_name
    )
}

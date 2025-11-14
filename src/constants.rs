//! 公共常量与复用函数
//! 提供：
//! - 合法日志级别常量 LOG_LEVELS
//! - 数据库表结构与 SQL 语句模板

/// 合法的日志级别（统一来源）
pub const LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];

/// 数据库表列定义（所有数据库导出器共用）
#[allow(dead_code)]
const DB_TABLE_COLUMNS: &str = "
    ts VARCHAR(64) NOT NULL,
    ep INTEGER NOT NULL,
    sess_id VARCHAR(64) NOT NULL,
    thrd_id VARCHAR(64) NOT NULL,
    username VARCHAR(128),
    trx_id VARCHAR(64),
    statement VARCHAR(128),
    appname VARCHAR(256),
    client_ip VARCHAR(64),
    body TEXT NOT NULL,
    replace_sql_parameters TEXT,
    exec_time_ms NUMBER(10, 2),
    row_count INTEGER,
    exec_id VARCHAR(64)
";

/// 生成 CREATE TABLE SQL 语句
#[cfg(feature = "sqlite")]
pub fn create_table_sql(table_name: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {} ({});",
        table_name, DB_TABLE_COLUMNS
    )
}

/// 生成 DROP TABLE SQL 语句
#[cfg(feature = "sqlite")]
pub fn drop_table_sql(table_name: &str) -> String {
    format!("DROP TABLE IF EXISTS {};", table_name)
}

/// 生成 INSERT SQL 语句
#[cfg(feature = "sqlite")]
pub fn insert_sql(table_name: &str) -> String {
    format!(
        "INSERT INTO {} (ts, ep, sess_id, thrd_id, username, trx_id, statement, \
         appname, client_ip, body, replace_sql_parameters, exec_time_ms, row_count, exec_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
        table_name
    )
}

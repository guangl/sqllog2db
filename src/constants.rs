//! 公共常量与复用函数
//! 提供：
//! - 合法日志级别常量 LOG_LEVELS
//! - 表结构字段模板 DB_TABLE_COLUMNS_SQL

/// 合法的日志级别（统一来源）
pub const LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];

/// 数据库表字段模板（DuckDB / SQLite 通用）
/// 使用 `format!(DB_TABLE_COLUMNS_SQL, table_name)` 不直接，因为包含 NOT NULL 等固定定义。
/// 仅导出列定义字符串，便于后续复用或扩展（例如添加索引）。
pub const DB_TABLE_COLUMNS_SQL: &str = r#"(
    ts TEXT NOT NULL,
    ep INTEGER NOT NULL,
    sess_id TEXT NOT NULL,
    thrd_id TEXT NOT NULL,
    username TEXT NOT NULL,
    trx_id TEXT NOT NULL,
    stmt_id TEXT NOT NULL,
    appname TEXT NOT NULL,
    body TEXT NOT NULL,
    replace_parameter_body TEXT,
    exec_time_ms REAL,
    row_count INTEGER,
    exec_id INTEGER
)"#;

/// 统一格式化创建表 SQL
pub fn create_table_sql(table: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {} {}",
        table, DB_TABLE_COLUMNS_SQL
    )
}

/// 统一格式化删除表 SQL
pub fn drop_table_sql(table: &str) -> String {
    format!("DROP TABLE IF EXISTS {}", table)
}

/// 统一格式化插入 SQL
pub fn insert_sql(table: &str) -> String {
    format!(
        "INSERT INTO {} (ts, ep, sess_id, thrd_id, username, trx_id, stmt_id, appname, body, replace_parameter_body, exec_time_ms, row_count, exec_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        table
    )
}

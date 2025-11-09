/// DM (达梦) 数据库 SQL 生成

/// 生成创建表的 SQL 语句
pub fn create_table_sql(table_name: &str) -> String {
    format!(
        r#"CREATE TABLE IF NOT EXISTS {} (
    ts DATETIME NOT NULL,
    ep INTEGER NOT NULL,
    sess_id BIGINT NOT NULL,
    thrd_id BIGINT NOT NULL,
    username VARCHAR(255) NOT NULL,
    trx_id BIGINT NOT NULL,
    stmt_id BIGINT NOT NULL,
    appname VARCHAR(255) NOT NULL,
    body CLOB NOT NULL,
    replace_parameter_body CLOB,
    exec_time_ms DOUBLE,
    row_count BIGINT,
    exec_id BIGINT
)"#,
        table_name
    )
}

/// 获取插入数据的 SQL 语句
pub fn insert_sql(table_name: &str) -> String {
    format!(
        r#"INSERT INTO {} (
    ts, ep, sess_id, thrd_id, username, trx_id, stmt_id, appname, body,
    replace_parameter_body, exec_time_ms, row_count, exec_id
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        table_name
    )
}

/// 获取删除表的 SQL 语句
pub fn drop_table_sql(table_name: &str) -> String {
    format!("DROP TABLE IF EXISTS {}", table_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_table_sql_contains_table_and_columns() {
        let sql = create_table_sql("dm_logs");
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS dm_logs"));
        assert!(sql.contains("ts DATETIME"));
        assert!(sql.contains("username VARCHAR"));
        assert!(sql.contains("exec_time_ms DOUBLE"));
    }

    #[test]
    fn test_insert_sql_placeholders_and_columns() {
        let sql = insert_sql("dm_logs");
        assert!(sql.starts_with("INSERT INTO dm_logs"));
        assert!(sql.contains("ts, ep, sess_id, thrd_id, username, trx_id, stmt_id, appname, body"));
        // 13 占位符
        let placeholders = sql.matches('?').count();
        assert_eq!(placeholders, 13);
    }

    #[test]
    fn test_drop_table_sql() {
        let sql = drop_table_sql("dm_logs");
        assert_eq!(sql, "DROP TABLE IF EXISTS dm_logs");
    }
}

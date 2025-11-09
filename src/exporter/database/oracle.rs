/// Oracle 数据库 SQL 生成

/// 生成创建表的 SQL 语句
pub fn create_table_sql(table_name: &str) -> String {
    format!(
        r#"CREATE TABLE {} (
    ts TIMESTAMP NOT NULL,
    ep NUMBER NOT NULL,
    sess_id NUMBER NOT NULL,
    thrd_id NUMBER NOT NULL,
    username VARCHAR2(255) NOT NULL,
    trx_id NUMBER NOT NULL,
    stmt_id NUMBER NOT NULL,
    appname VARCHAR2(255) NOT NULL,
    body CLOB NOT NULL,
    replace_parameter_body CLOB,
    exec_time_ms NUMBER,
    row_count NUMBER,
    exec_id NUMBER
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
    format!("DROP TABLE {} CASCADE CONSTRAINTS", table_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_table_sql_oracle() {
        let sql = create_table_sql("oracle_logs");
        assert!(sql.contains("CREATE TABLE oracle_logs"));
        assert!(sql.contains("ts TIMESTAMP"));
        assert!(sql.contains("body CLOB"));
        assert!(sql.contains("exec_id NUMBER"));
    }

    #[test]
    fn test_insert_sql_oracle() {
        let sql = insert_sql("oracle_logs");
        assert!(sql.starts_with("INSERT INTO oracle_logs"));
        // 验证列顺序部分字段
        assert!(sql.contains("ts, ep, sess_id, thrd_id, username"));
        // 13 占位符
        assert_eq!(sql.matches('?').count(), 13);
    }

    #[test]
    fn test_drop_table_sql_oracle() {
        let sql = drop_table_sql("oracle_logs");
        assert_eq!(sql, "DROP TABLE oracle_logs CASCADE CONSTRAINTS");
    }
}

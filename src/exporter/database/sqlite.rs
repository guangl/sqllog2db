use super::DatabaseType;
use crate::constants::{create_table_sql, drop_table_sql, insert_sql};
use crate::error::{Error, ExportError, Result};
use crate::exporter::{ExportStats, Exporter};
use dm_database_parser_sqllog::sqllog::Sqllog;
use rusqlite::{Connection, params_from_iter};
use std::path::Path;
use tracing::{debug, info};

/// SQLite 数据库导出器
pub struct SQLiteExporter {
    connection: Option<Connection>,
    path: String,
    table_name: String,
    overwrite: bool,
    batch_size: usize,
    buffer: Vec<Sqllog>,
    stats: ExportStats,
}

impl SQLiteExporter {
    /// 创建新的 SQLite 导出器
    #[allow(dead_code)]
    pub fn new(path: String, table_name: String, overwrite: bool) -> Self {
        Self::with_batch_size(path, table_name, overwrite, 1000)
    }

    /// 创建带有自定义批量大小的 SQLite 导出器
    pub fn with_batch_size(
        path: String,
        table_name: String,
        overwrite: bool,
        batch_size: usize,
    ) -> Self {
        Self {
            connection: None,
            path,
            table_name,
            overwrite,
            batch_size,
            buffer: Vec::new(),
            stats: ExportStats::default(),
        }
    }

    /// 生成创建表的 SQL 语句
    fn create_table_sql(&self) -> String {
        create_table_sql(&self.table_name)
    }

    /// 生成删除表的 SQL 语句
    fn drop_table_sql(&self) -> String {
        drop_table_sql(&self.table_name)
    }

    /// 生成插入数据的 SQL 语句
    fn insert_sql(&self) -> String {
        insert_sql(&self.table_name)
    }

    /// 刷新缓冲区到数据库
    fn flush_buffer(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        // 先生成 SQL 语句（避免借用冲突）
        let insert_sql = self.insert_sql();

        let conn = self.connection.as_mut().ok_or_else(|| {
            Error::Export(ExportError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: "数据库未连接".to_string(),
            })
        })?;

        let tx = conn.transaction().map_err(|e| {
            Error::Export(ExportError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("创建事务失败: {}", e),
            })
        })?;

        {
            let mut stmt = tx.prepare(&insert_sql).map_err(|e| {
                Error::Export(ExportError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("准备插入语句失败: {}", e),
                })
            })?;

            for sqllog in &self.buffer {
                let params: Vec<Box<dyn rusqlite::ToSql>> = vec![
                    Box::new(sqllog.ts.clone()),
                    Box::new(sqllog.meta.ep),
                    Box::new(sqllog.meta.sess_id.clone()),
                    Box::new(sqllog.meta.thrd_id.clone()),
                    Box::new(sqllog.meta.username.clone()),
                    Box::new(sqllog.meta.trxid.clone()),
                    Box::new(sqllog.meta.statement.clone()),
                    Box::new(sqllog.meta.appname.clone()),
                    Box::new(sqllog.body.clone()),
                    Box::new(None::<String>), // replace_parameter_body 暂未实现
                    Box::new(sqllog.indicators.as_ref().map(|i| i.execute_time)),
                    Box::new(sqllog.indicators.as_ref().map(|i| i.row_count)),
                    Box::new(sqllog.indicators.as_ref().map(|i| i.execute_id)),
                ];

                stmt.execute(params_from_iter(params.iter())).map_err(|e| {
                    Error::Export(ExportError::DatabaseExportFailed {
                        table_name: self.table_name.clone(),
                        reason: format!("插入数据失败: {}", e),
                    })
                })?;
            }
        }

        tx.commit().map_err(|e| {
            Error::Export(ExportError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("提交事务失败: {}", e),
            })
        })?;

        let count = self.buffer.len();
        debug!("刷新 {} 条记录到 SQLite 数据库", count);

        // 记录刷新统计
        self.stats.record_flush(count);

        self.buffer.clear();
        Ok(())
    }
}

impl Exporter for SQLiteExporter {
    fn initialize(&mut self) -> Result<()> {
        info!(
            "初始化 SQLite 数据库导出器: path={}, table={}, overwrite={}",
            self.path, self.table_name, self.overwrite
        );

        // 如果需要覆盖且文件存在，先删除
        if self.overwrite && Path::new(&self.path).exists() {
            std::fs::remove_file(&self.path).map_err(|e| {
                Error::Export(ExportError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("删除已存在的数据库文件失败: {}", e),
                })
            })?;
            info!("已删除旧的数据库文件: {}", self.path);
        }

        // 连接数据库
        let conn = Connection::open(&self.path).map_err(|e| {
            Error::Export(ExportError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("连接 SQLite 数据库失败: {}", e),
            })
        })?;

        // 如果需要覆盖且数据库已存在表（但文件未删除），删除表
        if self.overwrite {
            conn.execute(&self.drop_table_sql(), []).map_err(|e| {
                Error::Export(ExportError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("删除已存在的表失败: {}", e),
                })
            })?;
            debug!("已删除表: {}", self.table_name);
        }

        // 创建表
        conn.execute(&self.create_table_sql(), []).map_err(|e| {
            Error::Export(ExportError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("创建表失败: {}", e),
            })
        })?;
        info!("成功创建表: {}", self.table_name);

        self.connection = Some(conn);
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog) -> Result<()> {
        self.buffer.push(sqllog.clone());
        self.stats.record_success();

        // 达到批量大小时刷新
        if self.batch_size > 0 && self.buffer.len() >= self.batch_size {
            self.flush_buffer()?;
        }

        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[Sqllog]) -> Result<()> {
        for sqllog in sqllogs {
            self.export(sqllog)?;
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 刷新剩余数据
        self.flush_buffer()?;

        // 关闭连接
        if let Some(conn) = self.connection.take() {
            conn.close().map_err(|(_, e)| {
                Error::Export(ExportError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("关闭数据库连接失败: {}", e),
                })
            })?;
            info!("成功关闭 SQLite 数据库连接");
        }

        info!(
            "SQLite 导出完成: 成功={}, 失败={}, 跳过={}",
            self.stats.exported, self.stats.failed, self.stats.skipped
        );

        Ok(())
    }

    fn name(&self) -> &str {
        DatabaseType::SQLite.as_str()
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_database_parser_sqllog::sqllog::{IndicatorsParts, MetaParts, Sqllog};
    use std::fs;
    use tempfile::TempDir;

    fn create_test_sqllog(id: i64) -> Sqllog {
        Sqllog {
            ts: format!("2025-01-09 10:00:{:02}.000", id),
            meta: MetaParts {
                ep: 0,
                sess_id: format!("0x{:x}", id),
                thrd_id: format!("{}", id),
                username: "test_user".to_string(),
                trxid: format!("{}", id),
                statement: format!("0x{:x}", id),
                appname: "test_app".to_string(),
                client_ip: "127.0.0.1".to_string(),
            },
            body: format!("[SEL] SELECT {} FROM test_table", id),
            indicators: Some(IndicatorsParts {
                execute_time: 10.5 + id as f32,
                row_count: (100 + id) as u32,
                execute_id: id,
            }),
        }
    }

    #[test]
    fn test_sqlite_exporter_new() {
        let exporter = SQLiteExporter::new("test.db".to_string(), "test_logs".to_string(), true);

        assert_eq!(exporter.path, "test.db");
        assert_eq!(exporter.table_name, "test_logs");
        assert!(exporter.overwrite);
        assert_eq!(exporter.batch_size, 1000);
        assert_eq!(exporter.buffer.len(), 0);
        assert!(exporter.connection.is_none());
    }

    #[test]
    fn test_sqlite_exporter_with_batch_size() {
        let exporter =
            SQLiteExporter::with_batch_size("test.db".to_string(), "logs".to_string(), false, 500);

        assert_eq!(exporter.batch_size, 500);
        assert!(!exporter.overwrite);
    }

    #[test]
    fn test_create_table_sql() {
        let exporter = SQLiteExporter::new("test.db".to_string(), "my_table".to_string(), true);

        let sql = exporter.create_table_sql();
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS my_table"));
        assert!(sql.contains("ts TEXT NOT NULL"));
        assert!(sql.contains("ep INTEGER NOT NULL"));
        assert!(sql.contains("body TEXT NOT NULL"));
        assert!(sql.contains("replace_parameter_body TEXT"));
        assert!(sql.contains("exec_time_ms REAL"));
    }

    #[test]
    fn test_drop_table_sql() {
        let exporter = SQLiteExporter::new("test.db".to_string(), "my_table".to_string(), true);

        let sql = exporter.drop_table_sql();
        assert_eq!(sql, "DROP TABLE IF EXISTS my_table");
    }

    #[test]
    fn test_insert_sql() {
        let exporter = SQLiteExporter::new("test.db".to_string(), "logs".to_string(), false);

        let sql = exporter.insert_sql();
        assert!(sql.starts_with("INSERT INTO logs"));
        assert!(sql.contains("ts, ep, sess_id"));
        assert!(sql.contains("VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"));
    }

    #[test]
    fn test_sqlite_initialize_and_create_table() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut exporter = SQLiteExporter::new(
            db_path.to_str().unwrap().to_string(),
            "test_logs".to_string(),
            false,
        );

        // 初始化应该创建数据库和表
        exporter.initialize()?;
        assert!(db_path.exists());
        assert!(exporter.connection.is_some());

        // 验证表已创建（通过查询表结构）
        {
            let conn = exporter.connection.as_ref().unwrap();
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='test_logs'")
                .unwrap();
            let tables: Vec<String> = stmt
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();

            assert_eq!(tables.len(), 1);
            assert_eq!(tables[0], "test_logs");
        }

        exporter.finalize()?;
        Ok(())
    }

    #[test]
    fn test_sqlite_overwrite_mode() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // 第一次创建
        {
            let mut exporter = SQLiteExporter::new(
                db_path.to_str().unwrap().to_string(),
                "logs".to_string(),
                false,
            );
            exporter.initialize()?;
            exporter.export(&create_test_sqllog(1))?;
            exporter.finalize()?;
        }

        // 验证文件存在
        assert!(db_path.exists());
        let metadata1 = fs::metadata(&db_path).unwrap();

        // 使用 overwrite 模式再次创建
        {
            let mut exporter = SQLiteExporter::new(
                db_path.to_str().unwrap().to_string(),
                "logs".to_string(),
                true,
            );
            exporter.initialize()?;
            exporter.finalize()?;
        }

        // 文件应该被重新创建（大小可能不同）
        let metadata2 = fs::metadata(&db_path).unwrap();
        assert!(db_path.exists());
        // 新文件应该更小（没有数据）
        assert!(metadata2.len() <= metadata1.len());

        Ok(())
    }

    #[test]
    fn test_sqlite_export_single_record() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut exporter = SQLiteExporter::new(
            db_path.to_str().unwrap().to_string(),
            "logs".to_string(),
            true,
        );

        exporter.initialize()?;
        exporter.export(&create_test_sqllog(1))?;
        exporter.finalize()?;

        // 验证数据已插入
        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // 验证数据内容
        let body: String = conn
            .query_row("SELECT body FROM logs WHERE exec_id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(body, "[SEL] SELECT 1 FROM test_table");

        Ok(())
    }

    #[test]
    fn test_sqlite_export_batch() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut exporter = SQLiteExporter::new(
            db_path.to_str().unwrap().to_string(),
            "logs".to_string(),
            true,
        );

        exporter.initialize()?;

        // 批量导出 10 条记录
        let sqllogs: Vec<Sqllog> = (1..=10).map(create_test_sqllog).collect();
        exporter.export_batch(&sqllogs)?;
        exporter.finalize()?;

        // 验证数据
        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 10);

        // 验证统计
        assert_eq!(exporter.stats.exported, 10);

        Ok(())
    }

    #[test]
    fn test_sqlite_batch_flush() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // 设置批量大小为 5
        let mut exporter = SQLiteExporter::with_batch_size(
            db_path.to_str().unwrap().to_string(),
            "logs".to_string(),
            true,
            5,
        );

        exporter.initialize()?;

        // 导出 12 条记录，应该触发 2 次自动刷新（5+5），剩余 2 条在缓冲区
        for i in 1..=12 {
            exporter.export(&create_test_sqllog(i))?;
        }

        // 验证缓冲区
        assert_eq!(exporter.buffer.len(), 2);

        // 完成导出（刷新剩余数据）
        exporter.finalize()?;

        // 验证所有数据都已写入
        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 12);

        Ok(())
    }

    #[test]
    fn test_sqlite_with_null_indicators() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut exporter = SQLiteExporter::new(
            db_path.to_str().unwrap().to_string(),
            "logs".to_string(),
            true,
        );

        exporter.initialize()?;

        // 创建没有性能指标的记录
        let mut sqllog = create_test_sqllog(1);
        sqllog.indicators = None;

        exporter.export(&sqllog)?;
        exporter.finalize()?;

        // 验证数据已插入，性能指标字段为 NULL
        let conn = Connection::open(&db_path).unwrap();
        let exec_time: Option<f64> = conn
            .query_row("SELECT exec_time_ms FROM logs", [], |row| row.get(0))
            .unwrap();
        assert!(exec_time.is_none());

        Ok(())
    }

    #[test]
    fn test_sqlite_name() {
        let exporter = SQLiteExporter::new("test.db".to_string(), "logs".to_string(), false);

        assert_eq!(exporter.name(), "sqlite");
    }

    #[test]
    fn test_sqlite_stats() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut exporter = SQLiteExporter::new(
            db_path.to_str().unwrap().to_string(),
            "logs".to_string(),
            true,
        );

        exporter.initialize()?;

        // 初始统计
        assert_eq!(exporter.stats.exported, 0);
        assert_eq!(exporter.stats.failed, 0);
        assert_eq!(exporter.stats.skipped, 0);

        // 导出后统计
        for i in 1..=5 {
            exporter.export(&create_test_sqllog(i))?;
        }

        assert_eq!(exporter.stats.exported, 5);

        exporter.finalize()?;
        Ok(())
    }
}

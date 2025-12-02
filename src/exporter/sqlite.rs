use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use dm_database_parser_sqllog::Sqllog;
use log::{debug, info, warn};
use std::path::Path;

// 定义数据库表结构
table! {
    sqllog (id) {
        id -> Nullable<Integer>,
        ts -> Text,
        ep -> Integer,
        sess_id -> Text,
        thrd_id -> Text,
        username -> Text,
        trx_id -> Text,
        statement -> Text,
        appname -> Text,
        client_ip -> Text,
        sql -> Text,
        exec_time_ms -> Nullable<Float>,
        row_count -> Nullable<Integer>,
        exec_id -> Nullable<BigInt>,
    }
}

#[derive(Insertable)]
#[diesel(table_name = sqllog)]
struct NewSqllogRecord<'a> {
    ts: &'a str,
    ep: i32,
    sess_id: &'a str,
    thrd_id: &'a str,
    username: &'a str,
    trx_id: &'a str,
    statement: &'a str,
    appname: &'a str,
    client_ip: &'a str,
    sql: &'a str,
    exec_time_ms: Option<f32>,
    row_count: Option<i32>,
    exec_id: Option<i64>,
}

/// SQLite 导出器
pub struct SqliteExporter {
    database_url: String,
    pool: Option<Pool<ConnectionManager<SqliteConnection>>>,
    stats: ExportStats,
    batch_size: usize,
    pending_records: Vec<NewSqllogRecord<'static>>,
}

impl SqliteExporter {
    /// 创建新的 SQLite 导出器
    pub fn new(database_url: String, batch_size: usize) -> Self {
        Self {
            database_url,
            pool: None,
            stats: ExportStats::new(),
            batch_size,
            pending_records: Vec::with_capacity(batch_size),
        }
    }

    /// 从配置创建 SQLite 导出器
    pub fn from_config(config: &crate::config::SqliteExporter, batch_size: usize) -> Self {
        Self::new(config.database_url.clone(), batch_size)
    }

    /// 创建数据库表
    fn create_table(&self) -> Result<()> {
        let pool = self.pool.as_ref().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection pool not initialized".to_string(),
            })
        })?;

        let mut conn = pool.get().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to get connection: {}", e),
            })
        })?;

        diesel::sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS sqllog (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                ep INTEGER NOT NULL,
                sess_id TEXT NOT NULL,
                thrd_id TEXT NOT NULL,
                username TEXT NOT NULL,
                trx_id TEXT NOT NULL,
                statement TEXT NOT NULL,
                appname TEXT NOT NULL,
                client_ip TEXT NOT NULL,
                sql TEXT NOT NULL,
                exec_time_ms REAL,
                row_count INTEGER,
                exec_id INTEGER
            )
            "#,
        )
        .execute(&mut conn)
        .map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to create table: {}", e),
            })
        })?;

        info!("SQLite table created or already exists");
        Ok(())
    }

    /// 刷新待处理记录到数据库
    fn flush(&mut self) -> Result<()> {
        if self.pending_records.is_empty() {
            return Ok(());
        }

        let pool = self.pool.as_ref().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection pool not initialized".to_string(),
            })
        })?;

        let mut conn = pool.get().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to get connection: {}", e),
            })
        })?;

        let count = self.pending_records.len();

        diesel::insert_into(sqllog::table)
            .values(&self.pending_records)
            .execute(&mut conn)
            .map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to insert records: {}", e),
                })
            })?;

        debug!("Flushed {} records to SQLite", count);
        self.stats.flush_operations += 1;
        self.stats.last_flush_size = count;
        self.pending_records.clear();

        Ok(())
    }

    /// 将 Sqllog 转换为数据库记录
    fn sqllog_to_record(sqllog: &Sqllog<'_>) -> NewSqllogRecord<'static> {
        let meta = sqllog.parse_meta();
        let ind = sqllog.parse_indicators();

        NewSqllogRecord {
            ts: Box::leak(sqllog.ts.to_string().into_boxed_str()),
            ep: meta.ep as i32,
            sess_id: Box::leak(meta.sess_id.to_string().into_boxed_str()),
            thrd_id: Box::leak(meta.thrd_id.to_string().into_boxed_str()),
            username: Box::leak(meta.username.to_string().into_boxed_str()),
            trx_id: Box::leak(meta.trxid.to_string().into_boxed_str()),
            statement: Box::leak(meta.statement.to_string().into_boxed_str()),
            appname: Box::leak(meta.appname.to_string().into_boxed_str()),
            client_ip: Box::leak(meta.client_ip.to_string().into_boxed_str()),
            sql: Box::leak(sqllog.body().to_string().into_boxed_str()),
            exec_time_ms: ind.as_ref().map(|i| i.execute_time),
            row_count: ind.as_ref().map(|i| i.row_count as i32),
            exec_id: ind.as_ref().map(|i| i.execute_id),
        }
    }
}

impl Exporter for SqliteExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing SQLite exporter: {}", self.database_url);

        // 确保目录存在
        let path = Path::new(&self.database_url);
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    Error::Export(ExportError::DatabaseError {
                        reason: format!("Failed to create directory: {}", e),
                    })
                })?;
            }
        }

        // 创建连接池
        let manager = ConnectionManager::<SqliteConnection>::new(&self.database_url);
        let pool = Pool::builder()
            .max_size(5)
            .build(manager)
            .map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to create connection pool: {}", e),
                })
            })?;

        self.pool = Some(pool);

        // 创建表
        self.create_table()?;

        info!("SQLite exporter initialized: {}", self.database_url);
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        let record = Self::sqllog_to_record(sqllog);
        self.pending_records.push(record);

        // 当达到批量大小时刷新
        if self.pending_records.len() >= self.batch_size {
            self.flush()?;
        }

        self.stats.record_success();
        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog<'_>]) -> Result<()> {
        debug!("Exporting {} records to SQLite in batch", sqllogs.len());

        for sqllog in sqllogs {
            self.export(sqllog)?;
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 刷新剩余记录
        self.flush()?;

        info!(
            "SQLite export finished: {} (success: {}, failed: {})",
            self.database_url, self.stats.exported, self.stats.failed
        );

        Ok(())
    }

    fn name(&self) -> &str {
        "SQLite"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl Drop for SqliteExporter {
    fn drop(&mut self) {
        if !self.pending_records.is_empty() {
            if let Err(e) = self.finalize() {
                warn!("SQLite exporter finalization on Drop failed: {}", e);
            }
        }
    }
}

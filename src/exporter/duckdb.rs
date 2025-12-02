use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use duckdb::{Connection, params};
use log::{debug, info, warn};
use std::path::Path;

/// DuckDB 导出器
pub struct DuckdbExporter {
    database_url: String,
    conn: Option<Connection>,
    stats: ExportStats,
    batch_size: usize,
    pending_records: Vec<DuckdbRecord>,
}

#[derive(Debug, Clone)]
struct DuckdbRecord {
    ts: String,
    ep: i32,
    sess_id: String,
    thrd_id: String,
    username: String,
    trx_id: String,
    statement: String,
    appname: String,
    client_ip: String,
    sql: String,
    exec_time_ms: Option<f32>,
    row_count: Option<i32>,
    exec_id: Option<i64>,
}

impl DuckdbExporter {
    /// 创建新的 DuckDB 导出器
    pub fn new(database_url: String, batch_size: usize) -> Self {
        Self {
            database_url,
            conn: None,
            stats: ExportStats::new(),
            batch_size,
            pending_records: Vec::with_capacity(batch_size),
        }
    }

    /// 从配置创建 DuckDB 导出器
    pub fn from_config(config: &crate::config::DuckdbExporter, batch_size: usize) -> Self {
        Self::new(config.database_url.clone(), batch_size)
    }

    /// 创建数据库表
    fn create_table(&self) -> Result<()> {
        let conn = self.conn.as_ref().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS sqllog (
                id INTEGER PRIMARY KEY,
                ts VARCHAR NOT NULL,
                ep INTEGER NOT NULL,
                sess_id VARCHAR NOT NULL,
                thrd_id VARCHAR NOT NULL,
                username VARCHAR NOT NULL,
                trx_id VARCHAR NOT NULL,
                statement VARCHAR NOT NULL,
                appname VARCHAR NOT NULL,
                client_ip VARCHAR NOT NULL,
                sql TEXT NOT NULL,
                exec_time_ms FLOAT,
                row_count INTEGER,
                exec_id BIGINT
            )
            "#,
            [],
        )
        .map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to create table: {}", e),
            })
        })?;

        info!("DuckDB table created or already exists");
        Ok(())
    }

    /// 刷新待处理记录到数据库
    fn flush(&mut self) -> Result<()> {
        if self.pending_records.is_empty() {
            return Ok(());
        }

        let conn = self.conn.as_ref().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        let count = self.pending_records.len();

        // 使用事务批量插入
        let tx = conn.transaction().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to start transaction: {}", e),
            })
        })?;

        {
            let mut stmt = tx
                .prepare(
                    r#"
                    INSERT INTO sqllog (ts, ep, sess_id, thrd_id, username, trx_id,
                                       statement, appname, client_ip, sql,
                                       exec_time_ms, row_count, exec_id)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .map_err(|e| {
                    Error::Export(ExportError::DatabaseError {
                        reason: format!("Failed to prepare statement: {}", e),
                    })
                })?;

            for record in &self.pending_records {
                stmt.execute(params![
                    record.ts,
                    record.ep,
                    record.sess_id,
                    record.thrd_id,
                    record.username,
                    record.trx_id,
                    record.statement,
                    record.appname,
                    record.client_ip,
                    record.sql,
                    record.exec_time_ms,
                    record.row_count,
                    record.exec_id,
                ])
                .map_err(|e| {
                    Error::Export(ExportError::DatabaseError {
                        reason: format!("Failed to execute insert: {}", e),
                    })
                })?;
            }
        }

        tx.commit().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to commit transaction: {}", e),
            })
        })?;

        debug!("Flushed {} records to DuckDB", count);
        self.stats.flush_operations += 1;
        self.stats.last_flush_size = count;
        self.pending_records.clear();

        Ok(())
    }

    /// 将 Sqllog 转换为数据库记录
    fn sqllog_to_record(sqllog: &Sqllog<'_>) -> DuckdbRecord {
        let meta = sqllog.parse_meta();
        let ind = sqllog.parse_indicators();

        DuckdbRecord {
            ts: sqllog.ts.to_string(),
            ep: meta.ep as i32,
            sess_id: meta.sess_id.to_string(),
            thrd_id: meta.thrd_id.to_string(),
            username: meta.username.to_string(),
            trx_id: meta.trxid.to_string(),
            statement: meta.statement.to_string(),
            appname: meta.appname.to_string(),
            client_ip: meta.client_ip.to_string(),
            sql: sqllog.body().to_string(),
            exec_time_ms: ind.as_ref().map(|i| i.execute_time),
            row_count: ind.as_ref().map(|i| i.row_count as i32),
            exec_id: ind.as_ref().map(|i| i.execute_id),
        }
    }
}

impl Exporter for DuckdbExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing DuckDB exporter: {}", self.database_url);

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

        // 创建连接
        let conn = Connection::open(&self.database_url).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to open database: {}", e),
            })
        })?;

        self.conn = Some(conn);

        // 创建表
        self.create_table()?;

        info!("DuckDB exporter initialized: {}", self.database_url);
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
        debug!("Exporting {} records to DuckDB in batch", sqllogs.len());

        for sqllog in sqllogs {
            self.export(sqllog)?;
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 刷新剩余记录
        self.flush()?;

        info!(
            "DuckDB export finished: {} (success: {}, failed: {})",
            self.database_url, self.stats.exported, self.stats.failed
        );

        Ok(())
    }

    fn name(&self) -> &str {
        "DuckDB"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl Drop for DuckdbExporter {
    fn drop(&mut self) {
        if !self.pending_records.is_empty() {
            if let Err(e) = self.finalize() {
                warn!("DuckDB exporter finalization on Drop failed: {}", e);
            }
        }
    }
}

use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{debug, info, warn};
use postgres::{Client, NoTls};

/// PostgreSQL 导出器
pub struct PostgresExporter {
    connection_string: String,
    client: Option<Client>,
    stats: ExportStats,
    batch_size: usize,
    pending_records: Vec<PostgresRecord>,
}

#[derive(Debug, Clone)]
struct PostgresRecord {
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

impl PostgresExporter {
    /// 创建新的 PostgreSQL 导出器
    pub fn new(connection_string: String, batch_size: usize) -> Self {
        Self {
            connection_string,
            client: None,
            stats: ExportStats::new(),
            batch_size,
            pending_records: Vec::with_capacity(batch_size),
        }
    }

    /// 从配置创建 PostgreSQL 导出器
    pub fn from_config(config: &crate::config::PostgresExporter, batch_size: usize) -> Self {
        Self::new(config.connection_string.clone(), batch_size)
    }

    /// 创建数据库表
    fn create_table(&mut self) -> Result<()> {
        let client = self.client.as_mut().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        client
            .execute(
                r#"
                CREATE TABLE IF NOT EXISTS sqllog (
                    id SERIAL PRIMARY KEY,
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
                    exec_time_ms REAL,
                    row_count INTEGER,
                    exec_id BIGINT
                )
                "#,
                &[],
            )
            .map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to create table: {}", e),
                })
            })?;

        info!("PostgreSQL table created or already exists");
        Ok(())
    }

    /// 刷新待处理记录到数据库
    fn flush(&mut self) -> Result<()> {
        if self.pending_records.is_empty() {
            return Ok(());
        }

        let client = self.client.as_mut().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        let count = self.pending_records.len();

        // 使用事务批量插入
        let mut tx = client.transaction().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to start transaction: {}", e),
            })
        })?;

        let stmt = tx
            .prepare(
                r#"
                INSERT INTO sqllog (ts, ep, sess_id, thrd_id, username, trx_id,
                                   statement, appname, client_ip, sql,
                                   exec_time_ms, row_count, exec_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                "#,
            )
            .map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to prepare statement: {}", e),
                })
            })?;

        for record in &self.pending_records {
            tx.execute(
                &stmt,
                &[
                    &record.ts,
                    &record.ep,
                    &record.sess_id,
                    &record.thrd_id,
                    &record.username,
                    &record.trx_id,
                    &record.statement,
                    &record.appname,
                    &record.client_ip,
                    &record.sql,
                    &record.exec_time_ms,
                    &record.row_count,
                    &record.exec_id,
                ],
            )
            .map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to execute insert: {}", e),
                })
            })?;
        }

        tx.commit().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to commit transaction: {}", e),
            })
        })?;

        debug!("Flushed {} records to PostgreSQL", count);
        self.stats.flush_operations += 1;
        self.stats.last_flush_size = count;
        self.pending_records.clear();

        Ok(())
    }

    /// 将 Sqllog 转换为数据库记录
    fn sqllog_to_record(sqllog: &Sqllog<'_>) -> PostgresRecord {
        let meta = sqllog.parse_meta();
        let ind = sqllog.parse_indicators();

        PostgresRecord {
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

impl Exporter for PostgresExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing PostgreSQL exporter");

        // 创建连接
        let client = Client::connect(&self.connection_string, NoTls).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to connect to database: {}", e),
            })
        })?;

        self.client = Some(client);

        // 创建表
        self.create_table()?;

        info!("PostgreSQL exporter initialized");
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
        debug!("Exporting {} records to PostgreSQL in batch", sqllogs.len());

        for sqllog in sqllogs {
            self.export(sqllog)?;
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 刷新剩余记录
        self.flush()?;

        info!(
            "PostgreSQL export finished (success: {}, failed: {})",
            self.stats.exported, self.stats.failed
        );

        Ok(())
    }

    fn name(&self) -> &str {
        "PostgreSQL"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl Drop for PostgresExporter {
    fn drop(&mut self) {
        if !self.pending_records.is_empty() {
            if let Err(e) = self.finalize() {
                warn!("PostgreSQL exporter finalization on Drop failed: {}", e);
            }
        }
    }
}

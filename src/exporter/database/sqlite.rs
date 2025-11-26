/// SQLite 导出器 - 使用批量事务优化插入性能
use crate::constants::{create_table_sql, drop_table_sql, insert_sql};
use crate::error::{DatabaseError, Error, Result};
use crate::exporter::{ExportStats, Exporter};
use dm_database_parser_sqllog::Sqllog;
use log::{debug, info};
use rusqlite::{Connection, params};
use std::path::Path;

/// SQLite 数据库导出器
pub struct SQLiteExporter {
    connection: Option<Connection>,
    path: String,
    table_name: String,
    overwrite: bool,
    append: bool,
    batch_size: usize,
    stats: ExportStats,
    pending_records: Vec<Sqllog>,
}

impl SQLiteExporter {
    /// 创建带有自定义批量大小的 SQLite 导出器
    pub fn with_batch_size(
        path: String,
        table_name: String,
        overwrite: bool,
        append: bool,
        batch_size: usize,
    ) -> Self {
        Self {
            connection: None,
            path,
            table_name,
            overwrite,
            append,
            batch_size,
            stats: ExportStats::new(),
            pending_records: Vec::with_capacity(batch_size),
        }
    }

    /// 刷新待处理的记录到数据库
    fn flush_pending(&mut self) -> Result<()> {
        if self.pending_records.is_empty() {
            return Ok(());
        }

        let conn = self.connection.as_mut().ok_or_else(|| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: "Database not connected".to_string(),
            })
        })?;

        let count = self.pending_records.len();
        let insert_sql = insert_sql(&self.table_name);

        // 使用事务批量插入
        let tx = conn.transaction().map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("Failed to create transaction: {}", e),
            })
        })?;

        {
            let mut stmt = tx.prepare(&insert_sql).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("Failed to prepare insert statement: {}", e),
                })
            })?;

            for record in &self.pending_records {
                stmt.execute(params![
                    record.ts,
                    record.meta.ep as i32,
                    record.meta.sess_id,
                    record.meta.thrd_id,
                    record.meta.username,
                    record.meta.trxid,
                    record.meta.statement,
                    record.meta.appname,
                    record.meta.client_ip,
                    record.body,
                    None::<String>, // parameters placeholder
                    record.indicators.as_ref().map(|i| i.execute_time),
                    record.indicators.as_ref().map(|i| i.row_count as i32),
                    record.indicators.as_ref().map(|i| i.execute_id),
                ])
                .map_err(|e| {
                    Error::Database(DatabaseError::DatabaseExportFailed {
                        table_name: self.table_name.clone(),
                        reason: format!("Failed to insert data: {}", e),
                    })
                })?;

                self.stats.record_success();
            }
        }

        tx.commit().map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("Failed to commit transaction: {}", e),
            })
        })?;

        debug!("SQLite flushed {} records", count);
        self.pending_records.clear();
        Ok(())
    }
}

impl Exporter for SQLiteExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing SQLite exporter: {}", self.path);

        // 确保父目录存在
        if let Some(parent) = Path::new(&self.path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("Failed to create directory: {}", e),
                })
            })?;
        }

        // 仅在非 append 模式且 overwrite 时删除旧文件
        if !self.append && self.overwrite && Path::new(&self.path).exists() {
            std::fs::remove_file(&self.path).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("Failed to remove old database file: {}", e),
                })
            })?;
        }

        // 打开数据库连接
        let conn = Connection::open(&self.path).map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("Failed to open database: {}", e),
            })
        })?;

        // 性能优化设置
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA locking_mode = EXCLUSIVE;
             PRAGMA temp_store = MEMORY;",
        )
        .map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("Failed to set PRAGMA: {}", e),
            })
        })?;

        // 仅在非 append 模式且 overwrite 时重建表
        if !self.append && self.overwrite {
            let drop_sql = drop_table_sql(&self.table_name);
            conn.execute(&drop_sql, []).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("Failed to drop table: {}", e),
                })
            })?;
            let create_sql = create_table_sql(&self.table_name);
            conn.execute(&create_sql, []).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("Failed to create table: {}", e),
                })
            })?;
        }

        self.connection = Some(conn);
        info!("SQLite exporter initialized");
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog) -> Result<()> {
        self.pending_records.push(sqllog.clone());

        if self.pending_records.len() >= self.batch_size {
            self.flush_pending()?;
        }

        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog]) -> Result<()> {
        for sqllog in sqllogs {
            self.pending_records.push((*sqllog).clone());

            if self.pending_records.len() >= self.batch_size {
                self.flush_pending()?;
            }
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 刷新剩余记录
        self.flush_pending()?;

        // 关闭连接
        if let Some(conn) = self.connection.take() {
            // 优化数据库
            conn.execute_batch("PRAGMA optimize;").ok();
            drop(conn);
        }

        info!("SQLite export finished: {} records", self.stats.exported);
        Ok(())
    }

    fn name(&self) -> &str {
        "SQLite"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

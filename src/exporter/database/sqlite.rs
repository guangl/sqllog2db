/// SQLite 导出器 - 使用批量事务优化插入性能
use crate::constants::{create_table_sql, drop_table_sql, insert_sql};
use crate::error::{DatabaseError, Error, Result};
use crate::exporter::{ExportStats, Exporter};
use dm_database_parser_sqllog::Sqllog;
use rusqlite::{Connection, params};
use std::path::Path;
use tracing::{debug, info};

/// SQLite 数据库导出器
pub struct SQLiteExporter {
    connection: Option<Connection>,
    path: String,
    table_name: String,
    overwrite: bool,
    batch_size: usize,
    stats: ExportStats,
    pending_records: Vec<Sqllog>,
}

impl SQLiteExporter {
    /// 创建新的 SQLite 导出器
    pub fn new(path: String, table_name: String, overwrite: bool) -> Self {
        Self::with_batch_size(path, table_name, overwrite, 10000)
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
                reason: "数据库未连接".to_string(),
            })
        })?;

        let count = self.pending_records.len();
        let insert_sql = insert_sql(&self.table_name);

        // 使用事务批量插入
        let tx = conn.transaction().map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("创建事务失败: {}", e),
            })
        })?;

        {
            let mut stmt = tx.prepare(&insert_sql).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("准备插入语句失败: {}", e),
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
                    record.body,
                    None::<String>, // parameters placeholder
                    record.indicators.as_ref().map(|i| i.execute_time),
                    record.indicators.as_ref().map(|i| i.row_count as i32),
                    record.indicators.as_ref().map(|i| i.execute_id),
                ])
                .map_err(|e| {
                    Error::Database(DatabaseError::DatabaseExportFailed {
                        table_name: self.table_name.clone(),
                        reason: format!("插入数据失败: {}", e),
                    })
                })?;

                self.stats.record_success();
            }
        }

        tx.commit().map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("提交事务失败: {}", e),
            })
        })?;

        debug!("SQLite 刷新 {} 条记录", count);
        self.pending_records.clear();
        Ok(())
    }
}

impl Exporter for SQLiteExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("初始化 SQLite 导出器: {}", self.path);

        // 确保父目录存在
        if let Some(parent) = Path::new(&self.path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Export(ExportError::FileCreateFailed {
                    path: parent.to_path_buf(),
                    reason: e.to_string(),
                })
            })?;
        }

        // 如果需要覆盖，删除旧文件
        if self.overwrite && Path::new(&self.path).exists() {
            std::fs::remove_file(&self.path).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("删除旧数据库文件失败: {}", e),
                })
            })?;
        }

        // 打开数据库连接
        let conn = Connection::open(&self.path).map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("打开数据库失败: {}", e),
            })
        })?;

        // 性能优化设置
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA temp_store = MEMORY;",
        )
        .map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("设置 PRAGMA 失败: {}", e),
            })
        })?;

        // 创建表
        if self.overwrite {
            let drop_sql = drop_table_sql(&self.table_name);
            conn.execute(&drop_sql, []).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("删除表失败: {}", e),
                })
            })?;
        }

        let create_sql = create_table_sql(&self.table_name);
        conn.execute(&create_sql, []).map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("创建表失败: {}", e),
            })
        })?;

        self.connection = Some(conn);
        info!("SQLite 导出器初始化完成");
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

        info!("SQLite 导出完成: {} 条记录", self.stats.exported);
        Ok(())
    }

    fn name(&self) -> &str {
        "SQLite"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

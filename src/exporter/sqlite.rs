use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{info, warn};
use rayon::prelude::*;
use rusqlite::{Connection, params};
use std::path::Path;

/// SQLite 导出器 - 直接插入版本 (高性能)
pub struct SqliteExporter {
    database_url: String,
    table_name: String,
    overwrite: bool,
    append: bool,
    conn: Option<Connection>,
    stats: ExportStats,
}

impl SqliteExporter {
    /// 创建新的 SQLite 导出器
    pub fn new(database_url: String, table_name: String, overwrite: bool, append: bool) -> Self {
        Self {
            database_url,
            table_name,
            overwrite,
            append,
            conn: None,
            stats: ExportStats::new(),
        }
    }

    /// 从配置创建 SQLite 导出器
    pub fn from_config(config: &crate::config::SqliteExporter) -> Self {
        Self::new(
            config.database_url.clone(),
            config.table_name.clone(),
            config.overwrite,
            config.append,
        )
    }

    /// 创建数据库表
    fn create_table(&self) -> Result<()> {
        let conn = self.conn.as_ref().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        let sql = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                ts TEXT NOT NULL,
                ep INTEGER NOT NULL,
                sess_id TEXT NOT NULL,
                thrd_id TEXT NOT NULL,
                username TEXT NOT NULL,
                trx_id TEXT NOT NULL,
                statement TEXT NOT NULL,
                appname TEXT,
                client_ip TEXT,
                sql TEXT NOT NULL,
                exec_time_ms REAL,
                row_count INTEGER,
                exec_id INTEGER
            )
            "#,
            self.table_name
        );

        conn.execute(&sql, []).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to create table: {}", e),
            })
        })?;

        info!("SQLite table created or already exists");
        Ok(())
    }
}

impl Exporter for SqliteExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing SQLite exporter: {}", self.database_url);

        // 确保目录存在
        let path = Path::new(&self.database_url);
        if let Some(parent) = path.parent().filter(|p| !p.exists()) {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to create directory: {}", e),
                })
            })?;
        }

        // 创建数据库连接
        let conn = Connection::open(&self.database_url).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to open database: {}", e),
            })
        })?;

        // 性能优化: 关闭同步和日志，使用内存模式
        conn.execute_batch(
            "PRAGMA journal_mode = OFF;
             PRAGMA synchronous = OFF;
             PRAGMA cache_size = 1000000;
             PRAGMA locking_mode = EXCLUSIVE;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 30000000000;
             PRAGMA page_size = 65536;
             PRAGMA threads = 4;",
        )
        .map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to set PRAGMAs: {}", e),
            })
        })?;

        self.conn = Some(conn);

        // 处理 overwrite/append 逻辑
        if self.overwrite {
            // 如果 overwrite=true，删除已存在的表
            let drop_sql = format!("DROP TABLE IF EXISTS {}", self.table_name);
            if let Some(conn) = &self.conn {
                conn.execute(&drop_sql, []).map_err(|e| {
                    Error::Export(ExportError::DatabaseError {
                        reason: format!("Failed to drop table: {}", e),
                    })
                })?;
                info!("Dropped existing table: {}", self.table_name);
            }
        } else if !self.append {
            // 如果 overwrite=false 且 append=false，清空表数据
            if let Some(conn) = &self.conn {
                let delete_sql = format!("DELETE FROM {}", self.table_name);
                // 尝试清空，如果表不存在则忽略错误
                let _ = conn.execute(&delete_sql, []);
                info!("Cleared existing data from table: {}", self.table_name);
            }
        }

        // 创建表
        self.create_table()?;

        // 开启事务
        if let Some(conn) = &self.conn {
            conn.execute_batch("BEGIN TRANSACTION;").map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to begin transaction: {}", e),
                })
            })?;
        }

        info!("SQLite exporter initialized: {}", self.database_url);
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        let conn = self.conn.as_ref().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        // 使用 prepare_cached 缓存预编译语句
        // 注意：这里每次都 format 字符串，但由于 table_name 不变，字符串内容不变，prepare_cached 会命中缓存
        let sql = format!(
            "INSERT INTO {} VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            self.table_name
        );

        let mut stmt = conn.prepare_cached(&sql).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to prepare statement: {}", e),
            })
        })?;

        let meta = sqllog.parse_meta();
        let indicators = sqllog.parse_indicators();

        let (exec_time, row_count, exec_id) = if let Some(ind) = indicators {
            (
                Some(ind.execute_time),
                Some(ind.row_count),
                Some(ind.execute_id),
            )
        } else {
            (None, None, None)
        };

        stmt.execute(params![
            sqllog.ts,
            meta.ep,
            meta.sess_id,
            meta.thrd_id,
            meta.username,
            meta.trxid,
            meta.statement,
            meta.appname,
            meta.client_ip,
            sqllog.body().as_ref(),
            exec_time,
            row_count,
            exec_id
        ])
        .map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to insert record: {}", e),
            })
        })?;

        self.stats.record_success();
        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog<'_>]) -> Result<()> {
        if sqllogs.is_empty() {
            return Ok(());
        }

        let conn = self.conn.as_ref().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        let sql = format!(
            "INSERT INTO {} VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            self.table_name
        );

        let mut stmt = conn.prepare_cached(&sql).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to prepare statement: {}", e),
            })
        })?;

        // 内存优化：流式处理避免峰值
        // 分块处理（每 500 条），避免存储大量中间记录
        const CHUNK_SIZE: usize = 500;
        for chunk in sqllogs.chunks(CHUNK_SIZE) {
            let records: Vec<_> = chunk
                .par_iter()
                .map(|sqllog| {
                    let meta = sqllog.parse_meta();
                    let indicators = sqllog.parse_indicators();
                    let (exec_time, row_count, exec_id) = if let Some(ind) = indicators {
                        (
                            Some(ind.execute_time),
                            Some(ind.row_count),
                            Some(ind.execute_id),
                        )
                    } else {
                        (None, None, None)
                    };
                    (
                        sqllog.ts.to_string(),
                        meta.ep,
                        meta.sess_id.to_string(),
                        meta.thrd_id.to_string(),
                        meta.username.to_string(),
                        meta.trxid.to_string(),
                        meta.statement.to_string(),
                        meta.appname.to_string(),
                        meta.client_ip.to_string(),
                        sqllog.body().to_string(),
                        exec_time,
                        row_count,
                        exec_id,
                    )
                })
                .collect();

            for (
                ts,
                ep,
                sess_id,
                thrd_id,
                username,
                trxid,
                statement,
                appname,
                client_ip,
                sql_body,
                exec_time,
                row_count,
                exec_id,
            ) in records
            {
                stmt.execute(params![
                    ts, ep, sess_id, thrd_id, username, trxid, statement, appname, client_ip,
                    sql_body, exec_time, row_count, exec_id
                ])
                .map_err(|e| {
                    Error::Export(ExportError::DatabaseError {
                        reason: format!("Failed to insert record: {}", e),
                    })
                })?;

                self.stats.record_success();
            }
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 提交事务
        if let Some(conn) = &self.conn {
            conn.execute_batch("COMMIT;").map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to commit transaction: {}", e),
                })
            })?;
        }

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
        // 如果连接存在且未显式 finalize (可能 panic 或提前退出)，尝试回滚或提交？
        // 这里不做复杂处理，依赖 OS 回收文件锁
        // 如果事务未提交，SQLite 会自动回滚
    }
}

use super::{ExportStats, Exporter, csv::CsvExporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{debug, info, warn};
use rusqlite::Connection;
use std::path::Path;
use tempfile::NamedTempFile;

/// SQLite 导出器 - 使用 CSV 批量导入
pub struct SqliteExporter {
    database_url: String,
    conn: Option<Connection>,
    stats: ExportStats,
    batch_size: usize,
    csv_exporter: Option<CsvExporter>,
    temp_csv: Option<NamedTempFile>,
}

impl SqliteExporter {
    /// 创建新的 SQLite 导出器
    pub fn new(database_url: String, batch_size: usize) -> Self {
        Self {
            database_url,
            conn: None,
            stats: ExportStats::new(),
            batch_size,
            csv_exporter: None,
            temp_csv: None,
        }
    }

    /// 从配置创建 SQLite 导出器
    pub fn from_config(config: &crate::config::SqliteExporter, batch_size: usize) -> Self {
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
            [],
        )
        .map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to create table: {}", e),
            })
        })?;

        info!("SQLite table created or already exists");
        Ok(())
    }

    /// 刷新待处理记录到数据库（使用 CSV 虚拟表导入）
    fn flush(&mut self) -> Result<()> {
        // 先刷新 CSV 导出器
        if let Some(csv_exporter) = &mut self.csv_exporter {
            <CsvExporter as Exporter>::finalize(csv_exporter)?;
        }

        let temp_csv = self.temp_csv.take().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "No temporary CSV file".to_string(),
            })
        })?;

        let csv_path = temp_csv.path().to_string_lossy().to_string();

        let conn = self.conn.as_ref().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        // 使用 SQLite 的 CSV 虚拟表功能直接导入
        let sql = format!(
            r#"INSERT INTO sqllog (ts, ep, sess_id, thrd_id, username, trx_id,
                                  statement, appname, client_ip, sql,
                                  exec_time_ms, row_count, exec_id)
               SELECT ts, ep, sess_id, thrd_id, username, trx_id,
                      statement, appname, client_ip, sql,
                      NULLIF(exec_time_ms, ''),
                      NULLIF(row_count, ''),
                      NULLIF(exec_id, '')
               FROM csvtab('{}', 'header=yes')"#,
            csv_path.replace('\\', "\\\\").replace('\'', "''")
        );

        let count = conn.execute(&sql, []).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to import CSV: {}", e),
            })
        })?;

        debug!("Flushed {} records to SQLite from CSV", count);
        self.stats.flush_operations += 1;
        self.stats.last_flush_size = count;

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

        self.conn = Some(conn);

        // 创建表
        self.create_table()?;

        // 创建临时 CSV 文件
        let temp_csv = NamedTempFile::new().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to create temp CSV file: {}", e),
            })
        })?;

        // 创建 CSV 导出器
        let csv_exporter = CsvExporter::with_batch_size(temp_csv.path(), true, self.batch_size);
        self.csv_exporter = Some(csv_exporter);
        self.temp_csv = Some(temp_csv);

        // 初始化 CSV 导出器
        if let Some(csv_exporter) = &mut self.csv_exporter {
            csv_exporter.initialize()?;
        }

        info!("SQLite exporter initialized: {}", self.database_url);
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        // 导出到临时 CSV
        if let Some(csv_exporter) = &mut self.csv_exporter {
            csv_exporter.export(sqllog)?;
        }

        self.stats.record_success();
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 从 CSV 批量导入
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
        if self.csv_exporter.is_some()
            && let Err(e) = self.finalize()
        {
            warn!("SQLite exporter finalization on Drop failed: {}", e);
        }
    }
}

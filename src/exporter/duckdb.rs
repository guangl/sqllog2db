use super::{ExportStats, Exporter, csv::CsvExporter};
use crate::config;
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use duckdb::Connection;
use log::{debug, info, warn};
use std::fs;
use std::path::{Path, PathBuf};

/// `DuckDB` 导出器 - 使用 CSV 批量导入
pub struct DuckdbExporter {
    database_url: String,
    table_name: String,
    overwrite: bool,
    append: bool,
    conn: Option<Connection>,
    stats: ExportStats,
    csv_exporter: Option<CsvExporter>,
    temp_csv_path: Option<PathBuf>,
}
/// `DuckDB` 导出器 - 使用 CSV 批量导入
impl std::fmt::Debug for DuckdbExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DuckdbExporter")
            .field("database_url", &self.database_url)
            .field("table_name", &self.table_name)
            .field("overwrite", &self.overwrite)
            .field("append", &self.append)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl DuckdbExporter {
    /// 创建新的 `DuckDB` 导出器
    #[must_use]
    pub fn new(database_url: String, table_name: String, overwrite: bool, append: bool) -> Self {
        Self {
            database_url,
            table_name,
            overwrite,
            append,
            conn: None,
            stats: ExportStats::new(),
            csv_exporter: None,
            temp_csv_path: None,
        }
    }

    /// 从配置创建 `DuckDB` 导出器
    #[must_use]
    pub fn from_config(config: &config::DuckdbExporter) -> Self {
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

        let create_table_sql = format!(
            r"CREATE TABLE IF NOT EXISTS {} (
                ts VARCHAR NOT NULL,
                ep INTEGER NOT NULL,
                sess_id VARCHAR NOT NULL,
                thrd_id VARCHAR NOT NULL,
                username VARCHAR NOT NULL,
                trx_id VARCHAR NOT NULL,
                statement VARCHAR NOT NULL,
                appname VARCHAR,
                client_ip VARCHAR,
                sql TEXT NOT NULL,
                exec_time_ms FLOAT,
                row_count INTEGER,
                exec_id BIGINT
            )",
            self.table_name
        );

        conn.execute(&create_table_sql, []).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to create table: {e}"),
            })
        })?;

        info!("DuckDB table created or already exists");
        Ok(())
    }
}

impl Exporter for DuckdbExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing DuckDB exporter: {}", self.database_url);

        // 确保目录存在
        let path = Path::new(&self.database_url);
        if let Some(parent) = path.parent().filter(|p| !p.exists()) {
            fs::create_dir_all(parent).map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to create directory: {e}"),
                })
            })?;
        }

        // 创建连接
        let conn = Connection::open(&self.database_url).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to open database: {e}"),
            })
        })?;

        self.conn = Some(conn);

        // 处理 overwrite 和 append 模式
        if self.overwrite {
            info!("Overwrite mode: dropping existing table if it exists");
            if let Some(ref conn) = self.conn {
                conn.execute(&format!("DROP TABLE IF EXISTS {}", self.table_name), [])
                    .map_err(|e| {
                        Error::Export(ExportError::DatabaseError {
                            reason: format!("Failed to drop table: {e}"),
                        })
                    })?;
            }
        } else if !self.append {
            info!("Truncate mode: clearing existing table data");
            if let Some(ref conn) = self.conn {
                conn.execute(&format!("DELETE FROM {}", self.table_name), [])
                    .map_err(|e| {
                        Error::Export(ExportError::DatabaseError {
                            reason: format!("Failed to truncate table: {e}"),
                        })
                    })?;
            }
        } else {
            info!("Append mode: keeping existing data");
        }

        // 创建表（如果不存在）
        self.create_table()?;

        // 创建临时 CSV 文件用于批量导入
        let temp_dir = std::env::temp_dir();
        let temp_csv_path = temp_dir.join(format!("duckdb_import_{}.csv", std::process::id()));

        let mut csv_exporter = CsvExporter::new(&temp_csv_path);
        csv_exporter.initialize()?;
        self.csv_exporter = Some(csv_exporter);
        self.temp_csv_path = Some(temp_csv_path);

        info!("DuckDB exporter initialized: {}", self.database_url);
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        // 导出到临时 CSV
        if let Some(csv_exporter) = &mut self.csv_exporter {
            csv_exporter.export(sqllog)?;
            self.stats.record_success();
        }
        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog<'_>]) -> Result<()> {
        debug!("Exporting {} records to DuckDB in batch", sqllogs.len());

        // 直接使用 CSV 导出器的批量导出
        if let Some(csv_exporter) = &mut self.csv_exporter {
            csv_exporter.export_batch(sqllogs)?;
            self.stats.exported += sqllogs.len();
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 先关闭 CSV 导出器
        if let Some(mut csv_exporter) = self.csv_exporter.take() {
            <CsvExporter as Exporter>::finalize(&mut csv_exporter)?;
        }

        // 获取 CSV 文件路径
        let csv_path = self.temp_csv_path.as_ref().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "No temporary CSV file".to_string(),
            })
        })?;

        info!(
            "Importing {} records from CSV to DuckDB...",
            self.stats.exported
        );

        // 关闭连接以释放数据库锁
        self.conn = None;

        // 使用 DuckDB CLI 执行导入（使用 std::process::Command）
        let csv_path_str = csv_path.to_string_lossy().replace('\\', "/");
        let sql = format!(
            "PRAGMA threads=16; PRAGMA memory_limit='8GB'; SET preserve_insertion_order=false; COPY {} FROM '{}' (HEADER true, DELIMITER ',', PARALLEL true)",
            self.table_name,
            csv_path_str.replace('\'', "''")
        );

        let output = std::process::Command::new("duckdb")
            .arg(&self.database_url)
            .arg("-c")
            .arg(&sql)
            .output()
            .map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to execute duckdb CLI: {e}"),
                })
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Export(ExportError::DatabaseError {
                reason: format!("DuckDB import failed: {stderr}"),
            }));
        }

        info!(
            "Successfully imported {} records to DuckDB",
            self.stats.exported
        );

        // 清理临时文件
        if csv_path.exists() {
            let _ = fs::remove_file(csv_path);
        }
        self.temp_csv_path = None;

        info!(
            "DuckDB export finished: {} (success: {}, failed: {})",
            self.database_url, self.stats.exported, self.stats.failed
        );

        Ok(())
    }

    fn name(&self) -> &'static str {
        "DuckDB"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl Drop for DuckdbExporter {
    fn drop(&mut self) {
        // 仅当 CSV 导出器仍存在时才尝试 finalize
        if self.csv_exporter.is_some()
            && let Err(e) = self.finalize()
        {
            warn!("DuckDB exporter finalization on Drop failed: {e}");
        }
    }
}

use super::{ExportStats, Exporter, csv::CsvExporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{debug, info, warn};
use postgres::{Client, NoTls};
use std::io::Write;
use tempfile::NamedTempFile;

/// PostgreSQL 导出器 - 使用 CSV + COPY FROM STDIN
pub struct PostgresExporter {
    connection_string: String,
    schema: String,
    table_name: String,
    overwrite: bool,
    append: bool,
    client: Option<Client>,
    stats: ExportStats,
    batch_size: usize,
    csv_exporter: Option<CsvExporter>,
    temp_csv: Option<NamedTempFile>,
}

impl PostgresExporter {
    /// 创建新的 PostgreSQL 导出器
    pub fn new(
        connection_string: String,
        schema: String,
        table_name: String,
        overwrite: bool,
        append: bool,
        batch_size: usize,
    ) -> Self {
        Self {
            connection_string,
            schema,
            table_name,
            overwrite,
            append,
            client: None,
            stats: ExportStats::new(),
            batch_size,
            csv_exporter: None,
            temp_csv: None,
        }
    }

    /// 从配置创建 PostgreSQL 导出器
    pub fn from_config(config: &crate::config::PostgresExporter, batch_size: usize) -> Self {
        Self::new(
            config.connection_string(),
            config.schema.clone(),
            config.table_name.clone(),
            config.overwrite,
            config.append,
            batch_size,
        )
    }

    /// 获取完整表名
    fn full_table_name(&self) -> String {
        format!("{}.{}", self.schema, self.table_name)
    }

    /// 创建数据库表
    fn create_table(&mut self) -> Result<()> {
        let full_table_name = self.full_table_name();
        let client = self.client.as_mut().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        let sql = format!(
            r#"
                CREATE TABLE IF NOT EXISTS {} (
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
            full_table_name
        );

        client.execute(&sql, &[]).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to create table: {}", e),
            })
        })?;

        info!("PostgreSQL table created or already exists");
        Ok(())
    }

    /// 刷新待处理记录到数据库（使用 COPY FROM STDIN）
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

        let full_table_name = self.full_table_name();
        let client = self.client.as_mut().ok_or_else(|| {
            Error::Export(ExportError::DatabaseError {
                reason: "Connection not initialized".to_string(),
            })
        })?;

        // 读取 CSV 文件内容
        let csv_content = std::fs::read(temp_csv.path()).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to read CSV file: {}", e),
            })
        })?;

        // 使用 COPY FROM STDIN 导入
        let copy_sql = format!(
            r#"COPY {} (ts, ep, sess_id, thrd_id, username, trx_id,
                             statement, appname, client_ip, sql,
                             exec_time_ms, row_count, exec_id)
               FROM STDIN WITH (FORMAT CSV, HEADER true)"#,
            full_table_name
        );

        let mut writer = client.copy_in(&copy_sql).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to start COPY: {}", e),
            })
        })?;

        writer.write_all(&csv_content).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to write CSV data: {}", e),
            })
        })?;

        let count = writer.finish().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to finish COPY: {}", e),
            })
        })?;

        debug!("Flushed {} records to PostgreSQL from CSV", count);
        self.stats.flush_operations += 1;
        self.stats.last_flush_size = count as usize;

        Ok(())
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

        // 处理 overwrite/append 逻辑
        if self.overwrite {
            // 如果 overwrite=true，删除已存在的表
            let full_table_name = self.full_table_name();
            if let Some(client) = &mut self.client {
                let drop_sql = format!("DROP TABLE IF EXISTS {}", full_table_name);
                client.execute(&drop_sql, &[]).map_err(|e| {
                    Error::Export(ExportError::DatabaseError {
                        reason: format!("Failed to drop table: {}", e),
                    })
                })?;
                info!("Dropped existing table: {}", full_table_name);
            }
        } else if !self.append {
            // 如果 overwrite=false 且 append=false，清空表数据
            let full_table_name = self.full_table_name();
            if let Some(client) = &mut self.client {
                let delete_sql = format!("DELETE FROM {}", full_table_name);
                // 尝试清空，如果表不存在则忽略错误
                let _ = client.execute(&delete_sql, &[]);
                info!("Cleared existing data from table: {}", full_table_name);
            }
        }

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

        info!("PostgreSQL exporter initialized");
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
        if self.csv_exporter.is_some()
            && let Err(e) = self.finalize()
        {
            warn!("PostgreSQL exporter finalization on Drop failed: {}", e);
        }
    }
}

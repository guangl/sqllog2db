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
    ) -> Self {
        Self {
            connection_string,
            schema,
            table_name,
            overwrite,
            append,
            client: None,
            stats: ExportStats::new(),
            csv_exporter: None,
            temp_csv: None,
        }
    }

    /// 从配置创建 PostgreSQL 导出器
    pub fn from_config(config: &crate::config::PostgresExporter) -> Self {
        Self::new(
            config.connection_string(),
            config.schema.clone(),
            config.table_name.clone(),
            config.overwrite,
            config.append,
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
                    ts VARCHAR NOT NULL,
                    ep INTEGER NOT NULL,
                    sess_id VARCHAR,
                    thrd_id VARCHAR,
                    username VARCHAR,
                    trx_id VARCHAR,
                    statement VARCHAR,
                    appname VARCHAR,
                    client_ip VARCHAR,
                    sql TEXT,
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

        // 使用 COPY FROM STDIN 导入
        let copy_sql = format!(
            r#"COPY {} (ts, ep, sess_id, thrd_id, username, trx_id,
                             statement, appname, client_ip, sql,
                             exec_time_ms, row_count, exec_id)
               FROM STDIN WITH (FORMAT CSV, HEADER true)"#,
            full_table_name
        );

        info!(
            "Starting CSV import into PostgreSQL via COPY for table: {}",
            full_table_name
        );

        let mut writer = client.copy_in(&copy_sql).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to start COPY: {}", e),
            })
        })?;

        // 流式读取和写入 CSV 文件，分块处理避免内存占用过大
        use std::io::{BufReader, Read};
        let file = std::fs::File::open(temp_csv.path()).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to open CSV file: {}", e),
            })
        })?;

        let mut reader = BufReader::with_capacity(8 * 1024 * 1024, file); // 8MB buffer
        let mut buffer = vec![0u8; 1024 * 1024]; // 1MB chunks
        let mut total_bytes = 0usize;

        loop {
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to read CSV file: {}", e),
                })
            })?;

            if bytes_read == 0 {
                break;
            }

            writer.write_all(&buffer[..bytes_read]).map_err(|e| {
                Error::Export(ExportError::DatabaseError {
                    reason: format!("Failed to write CSV data: {}", e),
                })
            })?;

            total_bytes += bytes_read;
        }

        let count = writer.finish().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to finish COPY: {}", e),
            })
        })?;

        info!(
            "Finished CSV import into PostgreSQL: {} rows ({} bytes) committed",
            count, total_bytes
        );
        debug!(
            "Flushed {} records ({} bytes) to PostgreSQL from CSV",
            count, total_bytes
        );
        self.stats.flush_operations += 1;
        self.stats.last_flush_size = count as usize;

        Ok(())
    }
}

impl Exporter for PostgresExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing PostgreSQL exporter");

        // 输出连接字符串用于调试
        debug!("Connection string: {}", self.connection_string);

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
        let csv_exporter = CsvExporter::new(temp_csv.path(), true);
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

        // 成功后释放资源，避免 Drop 时重复 finalize 产生告警
        self.csv_exporter = None;
        self.temp_csv = None;

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
        // 仅当仍持有 CSV 导出器与临时文件时才尝试 finalize
        if self.csv_exporter.is_some() && self.temp_csv.is_some() {
            if let Err(e) = self.finalize() {
                warn!("PostgreSQL exporter finalization on Drop failed: {}", e);
            }
        }
    }
}

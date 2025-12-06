use super::{ExportStats, Exporter, csv::CsvExporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::{debug, info, warn};
use postgres::{Client, NoTls};
use tempfile::NamedTempFile;

/// PostgreSQL 导出器 - 使用 CSV + psql COPY FROM
pub struct PostgresExporter {
    connection_string: String,
    host: String,
    port: u16,
    username: String,
    password: String,
    database: String,
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        connection_string: String,
        host: String,
        port: u16,
        username: String,
        password: String,
        database: String,
        schema: String,
        table_name: String,
        overwrite: bool,
        append: bool,
    ) -> Self {
        Self {
            connection_string,
            host,
            port,
            username,
            password,
            database,
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
            config.host.clone(),
            config.port,
            config.username.clone(),
            config.password.clone(),
            config.database.clone(),
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
                CREATE UNLOGGED TABLE IF NOT EXISTS {} (
                    ts VARCHAR,
                    ep INTEGER,
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

    /// 刷新待处理记录到数据库（使用 psql COPY FROM）
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
        let csv_path = temp_csv.path().to_string_lossy().replace('\\', "/");

        info!(
            "Starting CSV import into PostgreSQL via psql COPY for table: {}",
            full_table_name
        );

        // 使用 psql 命令行工具执行 COPY FROM，比客户端传输快得多
        let copy_sql = format!(
            "\\COPY {} (ts, ep, sess_id, thrd_id, username, trx_id, statement, appname, client_ip, sql, exec_time_ms, row_count, exec_id) FROM '{}' WITH (FORMAT CSV, HEADER true)",
            full_table_name,
            csv_path.replace('\'', "''")
        );

        let mut cmd = std::process::Command::new("psql");
        cmd.arg("-h")
            .arg(&self.host)
            .arg("-p")
            .arg(self.port.to_string())
            .arg("-U")
            .arg(&self.username)
            .arg("-d")
            .arg(&self.database)
            .arg("-c")
            .arg(&copy_sql);

        // 如果有密码，通过环境变量传递
        if !self.password.is_empty() {
            cmd.env("PGPASSWORD", &self.password);
        }

        let output = cmd.output().map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to execute psql: {}", e),
            })
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Export(ExportError::DatabaseError {
                reason: format!("PostgreSQL import failed: {}", stderr),
            }));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        info!("PostgreSQL import completed: {}", stdout.trim());

        self.stats.flush_operations += 1;
        self.stats.last_flush_size = self.stats.exported;

        Ok(())
    }
}

impl Exporter for PostgresExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing PostgreSQL exporter");

        // 输出连接字符串用于调试
        debug!("Connection string: {}", self.connection_string);

        // 创建连接
        let mut client = Client::connect(&self.connection_string, NoTls).map_err(|e| {
            Error::Export(ExportError::DatabaseError {
                reason: format!("Failed to connect to database: {}", e),
            })
        })?;

        // 优化性能设置
        let _ = client.execute("SET synchronous_commit = OFF", &[]);
        let _ = client.execute("SET maintenance_work_mem = '2GB'", &[]);
        let _ = client.execute("SET work_mem = '512MB'", &[]);
        let _ = client.execute("SET max_parallel_workers_per_gather = 8", &[]);
        let _ = client.execute("SET max_parallel_workers = 16", &[]);
        let _ = client.execute("SET shared_buffers = '2GB'", &[]);

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

        // 创建临时 CSV 文件（使用当前目录以避免跨磁盘操作）
        let temp_csv = NamedTempFile::new_in("export")
            .map_err(|e| {
                // 如果 export 目录不存在，使用系统临时目录
                NamedTempFile::new().map_err(|e2| {
                    Error::Export(ExportError::DatabaseError {
                        reason: format!("Failed to create temp CSV file: {} ({})", e, e2),
                    })
                })
            })
            .or_else(|_| {
                NamedTempFile::new().map_err(|e| {
                    Error::Export(ExportError::DatabaseError {
                        reason: format!("Failed to create temp CSV file: {}", e),
                    })
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

    fn export_batch(&mut self, sqllogs: &[&Sqllog<'_>]) -> Result<()> {
        debug!("Exporting {} records to PostgreSQL in batch", sqllogs.len());

        // 直接使用 CSV 导出器的批量导出
        if let Some(csv_exporter) = &mut self.csv_exporter {
            csv_exporter.export_batch(sqllogs)?;
            self.stats.exported += sqllogs.len();
        }

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
        if self.csv_exporter.is_some() && self.temp_csv.is_some() && let Err(e) = self.finalize() {
            warn!("PostgreSQL exporter finalization on Drop failed: {}", e);
        }
    }
}

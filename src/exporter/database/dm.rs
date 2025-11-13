/// DM 数据库导出器 - 使用批量事务优化插入性能
use crate::constants::{create_table_sql, drop_table_sql, insert_sql};
use crate::error::{DatabaseError, Error, Result};
use crate::exporter::{ExportStats, Exporter};
use dameng::Connection;
use dameng::sql_type::ToSql;
use dm_database_parser_sqllog::Sqllog;
use tracing::{debug, info};

/// DM 数据库导出器
pub struct DmExporter {
    connection: Option<Connection>,
    host: String,
    port: u16,
    username: String,
    password: String,
    table_name: String,
    overwrite: bool,
    batch_size: usize,
    stats: ExportStats,
    pending_records: Vec<Sqllog>,
}

impl DmExporter {
    /// 创建新的 DM 导出器
    pub fn new(
        host: String,
        port: u16,
        username: String,
        password: String,
        table_name: String,
        overwrite: bool,
    ) -> Self {
        Self::with_batch_size(host, port, username, password, table_name, overwrite, 10000)
    }

    /// 创建带有自定义批量大小的 DM 导出器
    pub fn with_batch_size(
        host: String,
        port: u16,
        username: String,
        password: String,
        table_name: String,
        overwrite: bool,
        batch_size: usize,
    ) -> Self {
        Self {
            connection: None,
            host,
            port,
            username,
            password,
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
        conn.execute("BEGIN", &[]).map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("开始事务失败: {}", e),
            })
        })?;

        for record in &self.pending_records {
            // 预先创建临时值以延长生命周期
            let ep = record.meta.ep as i32;
            let execute_time = record.indicators.as_ref().map(|i| i.execute_time as i32);
            let row_count = record.indicators.as_ref().map(|i| i.row_count as i32);
            let execute_id = record.indicators.as_ref().map(|i| i.execute_id);

            let params: Vec<&dyn ToSql> = vec![
                &record.ts,
                &ep,
                &record.meta.sess_id,
                &record.meta.thrd_id,
                &record.meta.username,
                &record.meta.trxid,
                &record.meta.statement,
                &record.meta.appname,
                &record.meta.client_ip,
                &record.body,
                &execute_time,
                &row_count,
                &execute_id,
            ];

            conn.execute(&insert_sql, &params).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("插入数据失败: {}", e),
                })
            })?;

            self.stats.record_success();
        }

        conn.execute("COMMIT", &[]).map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("提交事务失败: {}", e),
            })
        })?;

        debug!("DM 刷新 {} 条记录", count);
        self.pending_records.clear();
        Ok(())
    }
}

impl Exporter for DmExporter {
    fn initialize(&mut self) -> Result<()> {
        info!(
            "初始化 DM 导出器: {}:{}/{}",
            self.host, self.port, self.table_name
        );

        // 构建连接字符串
        let connect_string = format!("{}:{}", self.host, self.port);

        // 连接数据库
        let conn =
            Connection::connect(&self.username, &self.password, &connect_string).map_err(|e| {
                Error::Database(DatabaseError::DatabaseExportFailed {
                    table_name: self.table_name.clone(),
                    reason: format!("连接数据库失败: {}", e),
                })
            })?;

        // 创建表
        if self.overwrite {
            let drop_sql = drop_table_sql(&self.table_name);
            // 忽略删除表的错误（表可能不存在）
            let _ = conn.execute(&drop_sql, &[]);
        }

        let create_sql = create_table_sql(&self.table_name);
        conn.execute(&create_sql, &[]).map_err(|e| {
            Error::Database(DatabaseError::DatabaseExportFailed {
                table_name: self.table_name.clone(),
                reason: format!("创建表失败: {}", e),
            })
        })?;

        self.connection = Some(conn);
        info!("DM 导出器初始化完成");
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
        if let Some(mut conn) = self.connection.take() {
            let _ = conn.close();
            drop(conn);
        }

        info!("DM 导出完成: {} 条记录", self.stats.exported);
        Ok(())
    }

    fn name(&self) -> &str {
        "DM"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

/// 数据库导出器实现
///
/// 支持多种数据库类型:
/// - DuckDB (文件型分析数据库) - 已实现
/// - SQLite (文件型嵌入式数据库) - 已实现
/// - PostgreSQL (网络型关系数据库) - 待实现
/// - Oracle (网络型企业数据库) - 待实现
/// - DM (达梦数据库) - 待实现
#[cfg(feature = "duckdb")]
mod duckdb;
#[cfg(feature = "oracle")]
mod oracle;
#[cfg(feature = "postgres")]
mod postgresql;
#[cfg(feature = "sqlite")]
mod sqlite;

use super::{ExportStats, Exporter};
use crate::config::DatabaseType;
use crate::error::Result;
use dm_database_parser_sqllog::Sqllog;
use tracing::{debug, info, warn};

/// 数据库连接枚举
enum DatabaseConnection {
    #[cfg(feature = "sqlite")]
    SQLite(sqlite::SQLiteExporter),
    #[cfg(feature = "duckdb")]
    DuckDB(duckdb::DuckDBExporter),
    // 其余数据库尚未实现或未启用 feature 时退化为 Unimplemented
    Unimplemented,
}

/// 数据库导出器（统一接口）
pub struct DatabaseExporter {
    database_type: DatabaseType,
    table_name: String,
    connection: DatabaseConnection,
}

impl DatabaseExporter {
    /// 从配置创建数据库导出器
    pub fn from_config(config: &crate::config::DatabaseExporter) -> Self {
        let table_name = config.table_name.clone();
        let database_type = config.database_type;

        #[cfg(any(feature = "sqlite", feature = "duckdb"))]
        let batch_size = config.batch_size;

        let connection = match database_type {
            DatabaseType::SQLite => {
                #[cfg(feature = "sqlite")]
                {
                    let path = config.path.as_deref().unwrap_or("sqllog.db");
                    DatabaseConnection::SQLite(sqlite::SQLiteExporter::with_batch_size(
                        path.to_string(),
                        table_name.clone(),
                        config.overwrite,
                        batch_size,
                    ))
                }
                #[cfg(not(feature = "sqlite"))]
                {
                    DatabaseConnection::Unimplemented
                }
            }
            DatabaseType::DuckDB => {
                #[cfg(feature = "duckdb")]
                {
                    let path = config.path.as_deref().unwrap_or("sqllog.duckdb");
                    DatabaseConnection::DuckDB(duckdb::DuckDBExporter::with_batch_size(
                        path.to_string(),
                        table_name.clone(),
                        config.overwrite,
                        batch_size,
                    ))
                }
                #[cfg(not(feature = "duckdb"))]
                {
                    DatabaseConnection::Unimplemented
                }
            }
            _ => DatabaseConnection::Unimplemented,
        };

        Self {
            database_type,
            table_name,
            connection,
        }
    }
}

impl Exporter for DatabaseExporter {
    fn initialize(&mut self) -> Result<()> {
        match &mut self.connection {
            #[cfg(feature = "sqlite")]
            DatabaseConnection::SQLite(exporter) => exporter.initialize(),
            #[cfg(feature = "duckdb")]
            DatabaseConnection::DuckDB(exporter) => exporter.initialize(),
            DatabaseConnection::Unimplemented => {
                info!(
                    "初始化 {} 数据库导出器(未启用特性或未实现): 表 = {}",
                    self.database_type.as_str(),
                    self.table_name
                );
                warn!("数据库导出器尚未启用或尚未实现,跳过实际连接");
                Ok(())
            }
        }
    }

    fn export(&mut self, #[allow(unused_variables)] sqllog: &Sqllog) -> Result<()> {
        match &mut self.connection {
            #[cfg(feature = "sqlite")]
            DatabaseConnection::SQLite(exporter) => exporter.export(sqllog),
            #[cfg(feature = "duckdb")]
            DatabaseConnection::DuckDB(exporter) => exporter.export(sqllog),
            DatabaseConnection::Unimplemented => Ok(()),
        }
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog]) -> Result<()> {
        match &mut self.connection {
            #[cfg(feature = "sqlite")]
            DatabaseConnection::SQLite(exporter) => exporter.export_batch(sqllogs),
            #[cfg(feature = "duckdb")]
            DatabaseConnection::DuckDB(exporter) => exporter.export_batch(sqllogs),
            DatabaseConnection::Unimplemented => {
                debug!(
                    "批量导出 {} 条记录到 {} (未启用/未实现,跳过)",
                    sqllogs.len(),
                    self.database_type.as_str()
                );
                Ok(())
            }
        }
    }

    fn finalize(&mut self) -> Result<()> {
        match &mut self.connection {
            #[cfg(feature = "sqlite")]
            DatabaseConnection::SQLite(exporter) => exporter.finalize(),
            #[cfg(feature = "duckdb")]
            DatabaseConnection::DuckDB(exporter) => exporter.finalize(),
            DatabaseConnection::Unimplemented => Ok(()),
        }
    }

    fn name(&self) -> &str {
        self.database_type.as_str()
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        match &self.connection {
            #[cfg(feature = "sqlite")]
            DatabaseConnection::SQLite(exporter) => exporter.stats_snapshot(),
            #[cfg(feature = "duckdb")]
            DatabaseConnection::DuckDB(exporter) => exporter.stats_snapshot(),
            DatabaseConnection::Unimplemented => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}

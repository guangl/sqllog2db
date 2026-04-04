use super::util::strip_ip_prefix;
use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::info;
use rusqlite::{Connection, params};
use std::path::Path;

pub struct SqliteExporter {
    database_url: String,
    table_name: String,
    insert_sql: String,
    overwrite: bool,
    append: bool,
    conn: Option<Connection>,
    stats: ExportStats,
    #[cfg(feature = "replace_parameters")]
    pub(super) normalize: bool,
}

impl std::fmt::Debug for SqliteExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteExporter")
            .field("database_url", &self.database_url)
            .field("table_name", &self.table_name)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl SqliteExporter {
    #[must_use]
    pub fn new(database_url: String, table_name: String, overwrite: bool, append: bool) -> Self {
        #[cfg(not(feature = "replace_parameters"))]
        let insert_sql =
            format!("INSERT INTO {table_name} VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)");
        #[cfg(feature = "replace_parameters")]
        let insert_sql = format!(
            "INSERT INTO {table_name} VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        );
        Self {
            database_url,
            table_name,
            insert_sql,
            overwrite,
            append,
            conn: None,
            stats: ExportStats::new(),
            #[cfg(feature = "replace_parameters")]
            normalize: true,
        }
    }

    #[must_use]
    pub fn from_config(config: &crate::config::SqliteExporter) -> Self {
        Self::new(
            config.database_url.clone(),
            config.table_name.clone(),
            config.overwrite,
            config.append,
        )
    }

    fn db_err(reason: impl Into<String>) -> Error {
        Error::Export(ExportError::DatabaseError {
            reason: reason.into(),
        })
    }

    fn do_insert(
        stmt: &mut rusqlite::CachedStatement<'_>,
        sqllog: &Sqllog<'_>,
        #[cfg(feature = "replace_parameters")] normalized_sql: Option<&str>,
    ) -> std::result::Result<(), rusqlite::Error> {
        let meta = sqllog.parse_meta();
        let pm = sqllog.parse_performance_metrics();
        let ind = sqllog.parse_indicators();
        let (exec_time, row_count, exec_id) = ind.map_or((None, None, None), |i| {
            (Some(i.exectime), Some(i.rowcount), Some(i.exec_id))
        });

        #[cfg(not(feature = "replace_parameters"))]
        stmt.execute(params![
            sqllog.ts.as_ref(),
            meta.ep,
            meta.sess_id.as_ref(),
            meta.thrd_id.as_ref(),
            meta.username.as_ref(),
            meta.trxid.as_ref(),
            meta.statement.as_ref(),
            meta.appname.as_ref(),
            strip_ip_prefix(meta.client_ip.as_ref()),
            sqllog.tag.as_deref(),
            pm.sql.as_ref(),
            exec_time,
            row_count,
            exec_id
        ])?;

        #[cfg(feature = "replace_parameters")]
        stmt.execute(params![
            sqllog.ts.as_ref(),
            meta.ep,
            meta.sess_id.as_ref(),
            meta.thrd_id.as_ref(),
            meta.username.as_ref(),
            meta.trxid.as_ref(),
            meta.statement.as_ref(),
            meta.appname.as_ref(),
            strip_ip_prefix(meta.client_ip.as_ref()),
            sqllog.tag.as_deref(),
            pm.sql.as_ref(),
            exec_time,
            row_count,
            exec_id,
            normalized_sql
        ])?;

        Ok(())
    }
}

impl Exporter for SqliteExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing SQLite exporter: {}", self.database_url);

        let path = Path::new(&self.database_url);
        if let Some(parent) = path.parent().filter(|p| !p.exists()) {
            std::fs::create_dir_all(parent)
                .map_err(|e| Self::db_err(format!("create dir failed: {e}")))?;
        }

        let conn = Connection::open(&self.database_url)
            .map_err(|e| Self::db_err(format!("open failed: {e}")))?;

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
        .map_err(|e| Self::db_err(format!("set PRAGMAs failed: {e}")))?;

        self.conn = Some(conn);

        if self.overwrite {
            let conn = self.conn.as_ref().unwrap();
            conn.execute(&format!("DROP TABLE IF EXISTS {}", self.table_name), [])
                .map_err(|e| Self::db_err(format!("drop table failed: {e}")))?;
            info!("Dropped existing table: {}", self.table_name);
        } else if !self.append {
            let conn = self.conn.as_ref().unwrap();
            let _ = conn.execute(&format!("DELETE FROM {}", self.table_name), []);
        }

        let conn = self.conn.as_ref().unwrap();
        #[cfg(not(feature = "replace_parameters"))]
        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                ts TEXT NOT NULL, ep INTEGER NOT NULL,
                sess_id TEXT NOT NULL, thrd_id TEXT NOT NULL,
                username TEXT NOT NULL, trx_id TEXT NOT NULL,
                statement TEXT, appname TEXT, client_ip TEXT, tag TEXT,
                sql TEXT NOT NULL,
                exec_time_ms REAL, row_count INTEGER, exec_id INTEGER
            )",
            self.table_name
        );
        #[cfg(feature = "replace_parameters")]
        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                ts TEXT NOT NULL, ep INTEGER NOT NULL,
                sess_id TEXT NOT NULL, thrd_id TEXT NOT NULL,
                username TEXT NOT NULL, trx_id TEXT NOT NULL,
                statement TEXT, appname TEXT, client_ip TEXT, tag TEXT,
                sql TEXT NOT NULL,
                exec_time_ms REAL, row_count INTEGER, exec_id INTEGER,
                normalized_sql TEXT
            )",
            self.table_name
        );
        conn.execute(&create_sql, [])
            .map_err(|e| Self::db_err(format!("create table failed: {e}")))?;

        conn.execute_batch("BEGIN TRANSACTION;")
            .map_err(|e| Self::db_err(format!("begin transaction failed: {e}")))?;

        info!("SQLite exporter initialized: {}", self.database_url);
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| Self::db_err("not initialized"))?;
        let mut stmt = conn
            .prepare_cached(&self.insert_sql)
            .map_err(|e| Self::db_err(format!("prepare failed: {e}")))?;
        Self::do_insert(
            &mut stmt,
            sqllog,
            #[cfg(feature = "replace_parameters")]
            None,
        )
        .map_err(|e| Self::db_err(format!("insert failed: {e}")))?;
        self.stats.record_success();
        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[Sqllog<'_>]) -> Result<()> {
        if sqllogs.is_empty() {
            return Ok(());
        }
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| Self::db_err("not initialized"))?;
        let mut stmt = conn
            .prepare_cached(&self.insert_sql)
            .map_err(|e| Self::db_err(format!("prepare failed: {e}")))?;
        for sqllog in sqllogs {
            Self::do_insert(
                &mut stmt,
                sqllog,
                #[cfg(feature = "replace_parameters")]
                None,
            )
            .map_err(|e| Self::db_err(format!("insert failed: {e}")))?;
        }
        self.stats.record_success_batch(sqllogs.len());
        Ok(())
    }

    #[cfg(feature = "replace_parameters")]
    fn export_batch_with_normalized(
        &mut self,
        sqllogs: &[Sqllog<'_>],
        normalized: &[Option<String>],
    ) -> Result<()> {
        if sqllogs.is_empty() {
            return Ok(());
        }
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| Self::db_err("not initialized"))?;
        let mut stmt = conn
            .prepare_cached(&self.insert_sql)
            .map_err(|e| Self::db_err(format!("prepare failed: {e}")))?;
        let normalize = self.normalize;
        for (sqllog, ns) in sqllogs.iter().zip(normalized.iter()) {
            let ns_ref = if normalize { ns.as_deref() } else { None };
            Self::do_insert(&mut stmt, sqllog, ns_ref)
                .map_err(|e| Self::db_err(format!("insert failed: {e}")))?;
        }
        self.stats.record_success_batch(sqllogs.len());
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        if let Some(conn) = &self.conn {
            conn.execute_batch("COMMIT;")
                .map_err(|e| Self::db_err(format!("commit failed: {e}")))?;
        }
        info!(
            "SQLite export finished: {} (success: {}, failed: {})",
            self.database_url, self.stats.exported, self.stats.failed
        );
        Ok(())
    }

    fn name(&self) -> &'static str {
        "SQLite"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

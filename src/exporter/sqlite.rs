use super::strip_ip_prefix;
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
        Error::Export(ExportError::DatabaseFailed {
            reason: reason.into(),
        })
    }

    fn do_insert(
        stmt: &mut rusqlite::CachedStatement<'_>,
        sqllog: &Sqllog<'_>,
        normalized_sql: Option<&str>,
    ) -> std::result::Result<(), rusqlite::Error> {
        let meta = sqllog.parse_meta();
        let pm = sqllog.parse_performance_metrics();
        let ind = sqllog.parse_indicators();
        let (exec_time, row_count, exec_id) = ind.map_or((None, None, None), |i| {
            (Some(i.exectime), Some(i.rowcount), Some(i.exec_id))
        });

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
        Self::do_insert(&mut stmt, sqllog, None)
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
            Self::do_insert(&mut stmt, sqllog, None)
                .map_err(|e| Self::db_err(format!("insert failed: {e}")))?;
        }
        self.stats.record_success_batch(sqllogs.len());
        Ok(())
    }

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
        Some(self.stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_database_parser_sqllog::LogParser;

    fn write_test_log(path: &std::path::Path, count: usize) {
        use std::fmt::Write as _;
        let mut buf = String::with_capacity(count * 170);
        for i in 0..count {
            writeln!(
                buf,
                "2025-01-15 10:30:28.001 (EP[0] sess:0x{i:04x} user:TESTUSER trxid:{i} stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT * FROM t WHERE id={i}. EXECTIME: {exec}(ms) ROWCOUNT: {rows}(rows) EXEC_ID: {i}.",
                exec = (i * 13) % 1000,
                rows = i % 100,
            ).unwrap();
        }
        std::fs::write(path, buf).unwrap();
    }

    #[test]
    fn test_sqlite_basic_export() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let dbfile = dir.path().join("out.db");
        write_test_log(&logfile, 5);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        {
            let mut exporter = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "sqllog_records".into(),
                true,
                false,
            );
            exporter.initialize().unwrap();
            exporter.export_batch(&records).unwrap();
            exporter.finalize().unwrap();
        } // exporter drops here, releasing EXCLUSIVE lock

        // Verify rows inserted
        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqllog_records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_sqlite_overwrite_drops_existing_table() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let dbfile = dir.path().join("out.db");
        write_test_log(&logfile, 3);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        // First run: insert 3 rows
        {
            let mut e =
                SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), false, false);
            e.initialize().unwrap();
            e.export_batch(&records).unwrap();
            e.finalize().unwrap();
        }

        // Second run with overwrite: should have only 3 rows again (not 6)
        {
            let mut e =
                SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), true, false);
            e.initialize().unwrap();
            e.export_batch(&records).unwrap();
            e.finalize().unwrap();
        }

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tbl", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_sqlite_with_normalized() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let dbfile = dir.path().join("out.db");
        write_test_log(&logfile, 2);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();
        let normalized: Vec<Option<String>> = records
            .iter()
            .map(|_| Some("SELECT * FROM t WHERE id=?".into()))
            .collect();

        {
            let mut exporter =
                SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), true, false);
            exporter.normalize = true;
            exporter.initialize().unwrap();
            exporter
                .export_batch_with_normalized(&records, &normalized)
                .unwrap();
            exporter.finalize().unwrap();
        } // exporter drops here, releasing EXCLUSIVE lock

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let ns: Option<String> = conn
            .query_row("SELECT normalized_sql FROM tbl LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ns, Some("SELECT * FROM t WHERE id=?".to_string()));
    }
}

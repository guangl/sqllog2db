use super::strip_ip_prefix;
use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::{MetaParts, PerformanceMetrics, Sqllog};
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
    row_count: usize,
    batch_size: usize,
    pub(super) normalize: bool,
    pub(super) field_mask: crate::features::FieldMask,
    pub(super) ordered_indices: Vec<usize>,
}

fn initialize_pragmas(conn: &Connection) -> std::result::Result<(), rusqlite::Error> {
    conn.execute_batch(
        "PRAGMA journal_mode = OFF;
         PRAGMA synchronous = OFF;
         PRAGMA cache_size = 1000000;
         PRAGMA locking_mode = EXCLUSIVE;
         PRAGMA temp_store = MEMORY;
         PRAGMA mmap_size = 30000000000;
         PRAGMA page_size = 65536;
         PRAGMA threads = 4;",
    )?;
    Ok(())
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
            "INSERT INTO \"{table_name}\" VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        );
        Self {
            database_url,
            table_name,
            insert_sql,
            overwrite,
            append,
            conn: None,
            stats: ExportStats::new(),
            row_count: 0,
            batch_size: 10_000,
            normalize: true,
            field_mask: crate::features::FieldMask::ALL,
            ordered_indices: (0..crate::features::FIELD_NAMES.len()).collect(),
        }
    }

    /// 根据有序字段索引列表生成 INSERT SQL
    fn build_insert_sql(table_name: &str, ordered_indices: &[usize]) -> String {
        use crate::features::FIELD_NAMES;
        if ordered_indices.len() == FIELD_NAMES.len() {
            // 全量快速路径：与 new() 的默认 insert_sql 一致
            return format!(
                "INSERT INTO \"{table_name}\" VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            );
        }
        let cols: Vec<&str> = ordered_indices.iter().map(|&i| FIELD_NAMES[i]).collect();
        let placeholders = vec!["?"; ordered_indices.len()].join(", ");
        format!(
            "INSERT INTO \"{table_name}\" ({}) VALUES ({placeholders})",
            cols.join(", ")
        )
    }

    /// 根据有序字段索引列表生成 CREATE TABLE SQL
    fn build_create_sql(table_name: &str, ordered_indices: &[usize]) -> String {
        use crate::features::FIELD_NAMES;
        const COL_TYPES: &[&str] = &[
            "TEXT NOT NULL",    // ts        0
            "INTEGER NOT NULL", // ep        1
            "TEXT NOT NULL",    // sess_id   2
            "TEXT NOT NULL",    // thrd_id   3
            "TEXT NOT NULL",    // username  4
            "TEXT NOT NULL",    // trx_id    5
            "TEXT",             // statement 6
            "TEXT",             // appname   7
            "TEXT",             // client_ip 8
            "TEXT",             // tag       9
            "TEXT NOT NULL",    // sql       10
            "INTEGER",          // exec_time_ms 11
            "INTEGER",          // row_count 12
            "INTEGER",          // exec_id   13
            "TEXT",             // normalized_sql 14
        ];
        let cols: Vec<String> = ordered_indices
            .iter()
            .map(|&i| format!("{} {}", FIELD_NAMES[i], COL_TYPES[i]))
            .collect();
        format!(
            "CREATE TABLE IF NOT EXISTS \"{table_name}\" ({})",
            cols.join(", ")
        )
    }

    #[must_use]
    pub fn from_config(config: &crate::config::SqliteExporter) -> Self {
        let mut exporter = Self::new(
            config.database_url.clone(),
            config.table_name.clone(),
            config.overwrite,
            config.append,
        );
        exporter.batch_size = config.batch_size;
        exporter
    }

    fn db_err(reason: impl Into<String>) -> Error {
        Error::Export(ExportError::DatabaseFailed {
            reason: reason.into(),
        })
    }

    /// 批量提交：每写入 `batch_size` 行后执行一次 `COMMIT; BEGIN`，
    /// 将大事务拆分为多个小事务，降低内存占用并提升写入稳定性。
    fn batch_commit_if_needed(&mut self) -> Result<()> {
        self.row_count += 1;
        if self.row_count % self.batch_size == 0 {
            let conn = self.conn.as_ref().unwrap();
            conn.execute_batch("COMMIT; BEGIN")
                .map_err(|e| Self::db_err(format!("batch commit failed: {e}")))?;
        }
        Ok(())
    }

    /// 热路径：使用预解析的 `MetaParts` 和 `PerformanceMetrics` 直接插入。
    /// 全量掩码走 `params![]` 快速路径；投影掩码走动态 Value 路径。
    ///
    /// 调用方通过 `prepare_cached()` 获取 `stmt`，利用 `StatementCache`（LRU，容量 16）
    /// 复用已编译的 statement，开销为 `RefCell::borrow_mut()` + `HashMap` lookup (O(1))，
    /// 而非 `sqlite3_prepare_v3()`（O(parse)）。PERF-06 满足。
    fn do_insert_preparsed(
        stmt: &mut rusqlite::CachedStatement<'_>,
        sqllog: &Sqllog<'_>,
        meta: &MetaParts<'_>,
        pm: &PerformanceMetrics<'_>,
        normalized_sql: Option<&str>,
        field_mask: crate::features::FieldMask,
        ordered_indices: &[usize],
    ) -> std::result::Result<(), rusqlite::Error> {
        let (exec_time_ms, row_count, exec_id) =
            if pm.exec_id != 0 || pm.exectime > 0.0 || pm.rowcount != 0 {
                // 与 CSV 路径保持一致：截断为整数毫秒（f32_ms_to_i64）
                (
                    Some(super::f32_ms_to_i64(pm.exectime)),
                    Some(pm.rowcount),
                    Some(pm.exec_id),
                )
            } else {
                (None, None, None)
            };

        if field_mask == crate::features::FieldMask::ALL {
            // 全量掩码快速路径：直接绑定全部 15 个参数
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
                exec_time_ms,
                row_count,
                exec_id,
                normalized_sql
            ])?;
            return Ok(());
        }

        // 投影路径：按有序索引从全量 Value 数组中选取（使用引用避免 move）
        use rusqlite::types::Value;
        let all: [Value; 15] = [
            Value::Text(sqllog.ts.as_ref().to_string()),
            Value::Integer(i64::from(meta.ep)),
            Value::Text(meta.sess_id.as_ref().to_string()),
            Value::Text(meta.thrd_id.as_ref().to_string()),
            Value::Text(meta.username.as_ref().to_string()),
            Value::Text(meta.trxid.as_ref().to_string()),
            Value::Text(meta.statement.as_ref().to_string()),
            Value::Text(meta.appname.as_ref().to_string()),
            Value::Text(strip_ip_prefix(meta.client_ip.as_ref()).to_string()),
            sqllog
                .tag
                .as_deref()
                .map_or(Value::Null, |t| Value::Text(t.to_string())),
            Value::Text(pm.sql.as_ref().to_string()),
            exec_time_ms.map_or(Value::Null, Value::Integer),
            row_count.map_or(Value::Null, |v| Value::Integer(i64::from(v))),
            exec_id.map_or(Value::Null, Value::Integer),
            normalized_sql.map_or(Value::Null, |s| Value::Text(s.to_string())),
        ];
        let selected: Vec<&Value> = ordered_indices.iter().map(|&i| &all[i]).collect();
        stmt.execute(rusqlite::params_from_iter(selected))?;
        Ok(())
    }

    /// 处理 `initialize()` 中 `DELETE FROM` 的执行结果（D-01 软失败语义）。
    /// - `Ok`：成功清空，无副作用
    /// - `SqliteFailure` 且 msg 含 "no such table"：首次运行预期，静默
    /// - 其他 `Err`：通过 `log::warn!` 记录，不向上传播错误（软失败）
    fn handle_delete_clear_result(result: rusqlite::Result<usize>, table_name: &str) {
        if let Err(rusqlite::Error::SqliteFailure(_, Some(ref msg))) = result {
            if msg.contains("no such table") {
                return;
            }
        }
        if let Err(e) = result {
            log::warn!("sqlite clear failed for table {table_name}: {e}");
        }
    }

    /// 根据 overwrite/append 模式准备目标表（清空或删除旧数据）。
    fn prepare_target_table(&self) -> Result<()> {
        if self.overwrite {
            let conn = self.conn.as_ref().unwrap();
            conn.execute(&format!("DROP TABLE IF EXISTS \"{}\"", self.table_name), [])
                .map_err(|e| Self::db_err(format!("drop table failed: {e}")))?;
            info!("Dropped existing table: {}", self.table_name);
        } else if !self.append {
            Self::handle_delete_clear_result(
                self.conn
                    .as_ref()
                    .unwrap()
                    .execute(&format!("DELETE FROM \"{}\"", self.table_name), []),
                &self.table_name,
            );
        }
        Ok(())
    }

    /// 兼容路径：从 `Sqllog` 内部解析再转调热路径（测试/批量导出使用）。
    fn do_insert(
        stmt: &mut rusqlite::CachedStatement<'_>,
        sqllog: &Sqllog<'_>,
        normalized_sql: Option<&str>,
        field_mask: crate::features::FieldMask,
        ordered_indices: &[usize],
    ) -> std::result::Result<(), rusqlite::Error> {
        let meta = sqllog.parse_meta();
        let pm = sqllog.parse_performance_metrics();
        Self::do_insert_preparsed(
            stmt,
            sqllog,
            &meta,
            &pm,
            normalized_sql,
            field_mask,
            ordered_indices,
        )
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

        initialize_pragmas(&conn).map_err(|e| Self::db_err(format!("set PRAGMAs failed: {e}")))?;

        self.conn = Some(conn);
        self.row_count = 0;

        self.prepare_target_table()?;

        // 根据 ordered_indices 重新生成 insert_sql（可在 new() 后被外部修改）
        self.insert_sql = Self::build_insert_sql(&self.table_name, &self.ordered_indices);

        let conn = self.conn.as_ref().unwrap();
        let create_sql = Self::build_create_sql(&self.table_name, &self.ordered_indices);
        conn.execute(&create_sql, [])
            .map_err(|e| Self::db_err(format!("create table failed: {e}")))?;

        conn.execute_batch("BEGIN TRANSACTION;")
            .map_err(|e| Self::db_err(format!("begin transaction failed: {e}")))?;

        info!("SQLite exporter initialized: {}", self.database_url);
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        {
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
                None,
                self.field_mask,
                &self.ordered_indices,
            )
            .map_err(|e| Self::db_err(format!("insert failed: {e}")))?;
        } // stmt and conn dropped here, releasing borrow
        self.stats.record_success();
        self.batch_commit_if_needed()?;
        Ok(())
    }

    fn export_one_normalized(
        &mut self,
        sqllog: &Sqllog<'_>,
        normalized: Option<&str>,
    ) -> Result<()> {
        {
            let conn = self
                .conn
                .as_ref()
                .ok_or_else(|| Self::db_err("not initialized"))?;
            let mut stmt = conn
                .prepare_cached(&self.insert_sql)
                .map_err(|e| Self::db_err(format!("prepare failed: {e}")))?;
            let ns_ref = if self.normalize { normalized } else { None };
            Self::do_insert(
                &mut stmt,
                sqllog,
                ns_ref,
                self.field_mask,
                &self.ordered_indices,
            )
            .map_err(|e| Self::db_err(format!("insert failed: {e}")))?;
        } // stmt and conn dropped here, releasing borrow
        self.stats.record_success();
        self.batch_commit_if_needed()?;
        Ok(())
    }

    fn export_one_preparsed(
        &mut self,
        sqllog: &Sqllog<'_>,
        meta: &MetaParts<'_>,
        pm: &PerformanceMetrics<'_>,
        normalized: Option<&str>,
    ) -> Result<()> {
        {
            let conn = self
                .conn
                .as_ref()
                .ok_or_else(|| Self::db_err("not initialized"))?;
            let mut stmt = conn
                .prepare_cached(&self.insert_sql)
                .map_err(|e| Self::db_err(format!("prepare failed: {e}")))?;
            let ns_ref = if self.normalize { normalized } else { None };
            Self::do_insert_preparsed(
                &mut stmt,
                sqllog,
                meta,
                pm,
                ns_ref,
                self.field_mask,
                &self.ordered_indices,
            )
            .map_err(|e| Self::db_err(format!("insert failed: {e}")))?;
        } // stmt and conn dropped here, releasing borrow
        self.stats.record_success();
        self.batch_commit_if_needed()?;
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

    fn write_template_stats(
        &mut self,
        stats: &[crate::features::TemplateStats],
        _final_path: Option<&std::path::Path>,
    ) -> Result<()> {
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| Self::db_err("write_template_stats: not initialized"))?;
        // BEGIN 必须在 DDL 之前：SQLite 支持事务性 DDL，将 DROP/CREATE 与 INSERT
        // 合并到同一事务内，确保任意步骤失败时均可整体回滚，不留半截建表结果。
        conn.execute_batch("BEGIN;")
            .map_err(|e| Self::db_err(format!("begin failed: {e}")))?;
        if self.overwrite {
            conn.execute("DROP TABLE IF EXISTS sql_templates", [])
                .map_err(|e| Self::db_err(format!("drop sql_templates failed: {e}")))?;
        }
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sql_templates \
             (template_key TEXT NOT NULL PRIMARY KEY, \
              count INTEGER NOT NULL, \
              avg_us INTEGER NOT NULL, \
              min_us INTEGER NOT NULL, \
              max_us INTEGER NOT NULL, \
              p50_us INTEGER NOT NULL, \
              p95_us INTEGER NOT NULL, \
              p99_us INTEGER NOT NULL, \
              first_seen TEXT NOT NULL, \
              last_seen TEXT NOT NULL)",
            [],
        )
        .map_err(|e| Self::db_err(format!("create sql_templates failed: {e}")))?;
        #[allow(clippy::cast_possible_wrap)]
        for s in stats {
            #[rustfmt::skip]
            let p = rusqlite::params![s.template_key, s.count as i64, s.avg_us as i64, s.min_us as i64, s.max_us as i64, s.p50_us as i64, s.p95_us as i64, s.p99_us as i64, s.first_seen, s.last_seen];
            conn.execute("INSERT INTO sql_templates VALUES (?,?,?,?,?,?,?,?,?,?)", p)
                .map_err(|e| Self::db_err(format!("insert sql_templates failed: {e}")))?;
        }
        conn.execute_batch("COMMIT;")
            .map_err(|e| Self::db_err(format!("commit sql_templates failed: {e}")))?;
        info!(
            "sql_templates: {} rows written to {}",
            stats.len(),
            self.database_url
        );
        Ok(())
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
            for r in &records {
                exporter.export_one_normalized(r, None).unwrap();
            }
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
            for r in &records {
                e.export_one_normalized(r, None).unwrap();
            }
            e.finalize().unwrap();
        }

        // Second run with overwrite: should have only 3 rows again (not 6)
        {
            let mut e =
                SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), true, false);
            e.initialize().unwrap();
            for r in &records {
                e.export_one_normalized(r, None).unwrap();
            }
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
            for (r, ns) in records.iter().zip(normalized.iter()) {
                exporter.export_one_normalized(r, ns.as_deref()).unwrap();
            }
            exporter.finalize().unwrap();
        } // exporter drops here, releasing EXCLUSIVE lock

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let ns: Option<String> = conn
            .query_row("SELECT normalized_sql FROM tbl LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ns, Some("SELECT * FROM t WHERE id=?".to_string()));
    }

    #[test]
    fn test_sqlite_from_config() {
        let dir = tempfile::TempDir::new().unwrap();
        let dbfile = dir.path().join("cfg.db");
        let cfg = crate::config::SqliteExporter {
            database_url: dbfile.to_string_lossy().into_owned(),
            table_name: "records".to_string(),
            overwrite: true,
            append: false,
            batch_size: 10_000,
        };
        let mut exporter = SqliteExporter::from_config(&cfg);
        exporter.initialize().unwrap();
        exporter.finalize().unwrap();
        assert!(dbfile.exists());
    }

    #[test]
    fn test_sqlite_export_method() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let dbfile = dir.path().join("export.db");
        write_test_log(&logfile, 3);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        {
            let mut exporter =
                SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), true, false);
            exporter.initialize().unwrap();
            for r in &records {
                // Use export() instead of export_one_normalized
                exporter.export(r).unwrap();
            }
            exporter.finalize().unwrap();
        }

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tbl", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_sqlite_export_one_preparsed() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let dbfile = dir.path().join("preparsed.db");
        write_test_log(&logfile, 2);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        {
            let mut exporter =
                SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), true, false);
            exporter.initialize().unwrap();
            for r in &records {
                let meta = r.parse_meta();
                let pm = r.parse_performance_metrics();
                exporter.export_one_preparsed(r, &meta, &pm, None).unwrap();
            }
            exporter.finalize().unwrap();
        }

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tbl", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_sqlite_stats_snapshot() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let dbfile = dir.path().join("stats.db");
        write_test_log(&logfile, 4);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        let mut exporter =
            SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), true, false);
        exporter.initialize().unwrap();
        for r in &records {
            exporter.export(r).unwrap();
        }
        let snap = exporter.stats_snapshot().unwrap();
        assert_eq!(snap.exported, 4);
        exporter.finalize().unwrap();
    }

    #[test]
    fn test_sqlite_debug_format() {
        let exporter =
            SqliteExporter::new("/tmp/debug.db".to_string(), "tbl".to_string(), true, false);
        let s = format!("{exporter:?}");
        assert!(s.contains("SqliteExporter"));
    }

    #[test]
    fn test_sqlite_build_insert_sql_ordered() {
        let sql = SqliteExporter::build_insert_sql("t", &[10, 4]);
        assert_eq!(sql, "INSERT INTO \"t\" (sql, username) VALUES (?, ?)");
    }

    #[test]
    fn test_sqlite_build_create_sql_ordered() {
        let sql = SqliteExporter::build_create_sql("t", &[10, 4]);
        assert_eq!(
            sql,
            "CREATE TABLE IF NOT EXISTS \"t\" (sql TEXT NOT NULL, username TEXT NOT NULL)"
        );
    }

    #[test]
    fn test_sqlite_build_insert_sql_full_fast_path() {
        let all_indices: Vec<usize> = (0..15).collect();
        let sql = SqliteExporter::build_insert_sql("t", &all_indices);
        assert_eq!(
            sql,
            "INSERT INTO \"t\" VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        );
    }

    #[test]
    fn test_sqlite_field_order() {
        use crate::features::FieldMask;

        let dir = tempfile::TempDir::new().unwrap();
        let log = dir.path().join("t.log");
        std::fs::write(
            &log,
            "2025-01-15 10:30:28.001 (EP[0] sess:0x0001 user:testuser trxid:1 stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT 42. EXECTIME: 5(ms) ROWCOUNT: 2(rows) EXEC_ID: 99.\n",
        )
        .unwrap();

        let db = dir.path().join("out.db");
        {
            let mut exporter = SqliteExporter::new(
                db.to_str().unwrap().to_string(),
                "records".to_string(),
                true,
                false,
            );
            exporter.normalize = false;
            exporter.field_mask =
                FieldMask::from_names(&["sql".to_string(), "username".to_string()]).unwrap();
            exporter.ordered_indices = vec![10, 4]; // sql=10, username=4
            exporter.initialize().unwrap();

            let parser = LogParser::from_path(log.to_str().unwrap()).unwrap();
            for record in parser.iter().flatten() {
                exporter.export(&record).unwrap();
            }
            exporter.finalize().unwrap();
        } // exporter drops here, releasing EXCLUSIVE lock

        let conn = rusqlite::Connection::open(&db).unwrap();
        let (sql_val, username_val): (String, String) = conn
            .query_row("SELECT sql, username FROM records", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();

        assert!(sql_val.contains("SELECT 42"), "sql_val: {sql_val}");
        assert_eq!(username_val, "testuser");
    }

    #[test]
    fn test_sqlite_append_mode() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let dbfile = dir.path().join("append.db");
        write_test_log(&logfile, 3);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        // First run: create table with 3 rows
        {
            let mut e =
                SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), false, false);
            e.initialize().unwrap();
            for r in &records {
                e.export(r).unwrap();
            }
            e.finalize().unwrap();
        }

        // Second run with append=true: adds 3 more rows
        {
            let mut e =
                SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), false, true);
            e.initialize().unwrap();
            for r in &records {
                e.export(r).unwrap();
            }
            e.finalize().unwrap();
        }

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tbl", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 6);
    }

    #[test]
    fn test_sqlite_initialize_creates_quoted_table() {
        let dir = tempfile::TempDir::new().unwrap();
        let dbfile = dir.path().join("quoted.db");
        {
            let mut exporter = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "my_records".to_string(),
                true,
                false,
            );
            exporter.initialize().unwrap();
            exporter.finalize().unwrap();
        }
        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let create_stmt: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='my_records'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            create_stmt.contains("\"my_records\""),
            "table name should be double-quoted; actual: {create_stmt}"
        );
    }

    #[test]
    fn test_sqlite_initialize_silent_when_table_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let dbfile = dir.path().join("missing_tbl.db");

        // overwrite=false 且 append=false → initialize 会走 DELETE FROM 分支
        // 由于 DB 全新，表不存在，DELETE 应触发 "no such table" 并被静默吃掉
        {
            let mut exporter = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "fresh_tbl".to_string(),
                false,
                false,
            );
            // 必须 Ok —— 不能因 "no such table" 返回 Err
            exporter
                .initialize()
                .expect("initialize should silently succeed when table missing");
            exporter.finalize().unwrap();
        } // exporter drops here, releasing EXCLUSIVE lock

        // 表应已被 CREATE TABLE IF NOT EXISTS 创建
        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='fresh_tbl'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "table fresh_tbl should be created");
    }

    #[test]
    fn test_sqlite_initialize_clears_existing_table_via_delete() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("test.log");
        let dbfile = dir.path().join("clear.db");
        write_test_log(&logfile, 4);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        // 第一次 run：写入 4 条
        {
            let mut e = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "clr_tbl".into(),
                false,
                false,
            );
            e.initialize().unwrap();
            for r in &records {
                e.export(r).unwrap();
            }
            e.finalize().unwrap();
        }

        // 第二次 run：overwrite=false、append=false → 走 DELETE FROM 清空已有数据
        // 然后写入同样 4 条 —— 期望最终行数 == 4 而非 8
        {
            let mut e = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "clr_tbl".into(),
                false,
                false,
            );
            e.initialize().unwrap();
            for r in &records {
                e.export(r).unwrap();
            }
            e.finalize().unwrap();
        }

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clr_tbl", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 4,
            "DELETE FROM should clear previous rows; got {count}"
        );
    }

    #[test]
    fn test_sqlite_batch_commit() {
        let dir = tempfile::TempDir::new().unwrap();
        let logfile = dir.path().join("batch.log");
        let dbfile = dir.path().join("batch.db");
        write_test_log(&logfile, 5);

        let parser = LogParser::from_path(logfile.to_str().unwrap()).unwrap();
        let records: Vec<_> = parser.iter().filter_map(std::result::Result::ok).collect();

        {
            let mut exporter =
                SqliteExporter::new(dbfile.to_string_lossy().into(), "tbl".into(), true, false);
            // batch_size=2：每 2 条触发一次中间 COMMIT（5 条 → 2 次中间 COMMIT，finalize 提交第5条）
            exporter.batch_size = 2;
            exporter.initialize().unwrap();
            for r in &records {
                exporter.export_one_normalized(r, None).unwrap();
            }
            exporter.finalize().unwrap();
        }

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tbl", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 5,
            "5 条记录经过批量提交后必须全部持久化，实际: {count}"
        );
    }

    /// 辅助：构造 `TemplateStats` 测试数据
    fn make_template_stats_sqlite(key: &str) -> crate::features::TemplateStats {
        crate::features::TemplateStats {
            template_key: key.to_string(),
            count: 5,
            avg_us: 100,
            min_us: 50,
            max_us: 200,
            p50_us: 90,
            p95_us: 180,
            p99_us: 195,
            first_seen: "2025-01-15 10:00:00".to_string(),
            last_seen: "2025-01-15 10:05:00".to_string(),
        }
    }

    /// TMPL-04-A：基本写入验证 — initialize → finalize → `write_template_stats` → 验证 COUNT=2
    #[test]
    fn test_sqlite_write_template_stats() {
        let dir = tempfile::TempDir::new().unwrap();
        let dbfile = dir.path().join("out.db");

        let stats = vec![
            make_template_stats_sqlite("SELECT 1"),
            make_template_stats_sqlite("SELECT 2"),
        ];

        {
            let mut exporter = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "sqllog_records".into(),
                true,
                false,
            );
            exporter.initialize().unwrap();
            exporter.finalize().unwrap();
            exporter.write_template_stats(&stats, None).unwrap();
        } // exporter drops here, releasing EXCLUSIVE lock

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sql_templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2, "expected 2 rows in sql_templates, got {count}");

        let (key, row_count, avg_us, p99_us, first_seen): (String, i64, i64, i64, String) = conn
            .query_row(
                "SELECT template_key, count, avg_us, p99_us, first_seen \
                 FROM sql_templates ORDER BY template_key LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(key, "SELECT 1");
        assert_eq!(row_count, 5);
        assert_eq!(avg_us, 100);
        assert_eq!(p99_us, 195);
        assert_eq!(first_seen, "2025-01-15 10:00:00");
    }

    /// TMPL-04-E：overwrite 覆盖 — 旧行被 DROP，只保留新行
    #[test]
    fn test_sqlite_templates_overwrite() {
        let dir = tempfile::TempDir::new().unwrap();
        let dbfile = dir.path().join("overwrite.db");

        // 第一次写入：overwrite=true，写入 "OLD"
        {
            let mut exporter = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "sqllog_records".into(),
                true,
                false,
            );
            exporter.initialize().unwrap();
            exporter.finalize().unwrap();
            exporter
                .write_template_stats(&[make_template_stats_sqlite("OLD")], None)
                .unwrap();
        }

        // 第二次写入：overwrite=true，写入 "NEW"（应 DROP 旧表）
        {
            let mut exporter = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "sqllog_records".into(),
                true,
                false,
            );
            exporter.initialize().unwrap();
            exporter.finalize().unwrap();
            exporter
                .write_template_stats(&[make_template_stats_sqlite("NEW")], None)
                .unwrap();
        }

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sql_templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 1,
            "overwrite should leave exactly 1 row, got {count}"
        );

        let key: String = conn
            .query_row("SELECT template_key FROM sql_templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(key, "NEW", "only NEW row should remain after overwrite");
    }

    /// TMPL-04-F：append 累加 — 旧行保留，新行累加
    #[test]
    fn test_sqlite_templates_append() {
        let dir = tempfile::TempDir::new().unwrap();
        let dbfile = dir.path().join("append_tpl.db");

        // 第一次写入：overwrite=true，写入 "A"
        {
            let mut exporter = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "sqllog_records".into(),
                true,
                false,
            );
            exporter.initialize().unwrap();
            exporter.finalize().unwrap();
            exporter
                .write_template_stats(&[make_template_stats_sqlite("A")], None)
                .unwrap();
        }

        // 第二次写入：overwrite=false、append=true，写入 "B"（应保留 "A"）
        {
            let mut exporter = SqliteExporter::new(
                dbfile.to_string_lossy().into(),
                "sqllog_records".into(),
                false,
                true,
            );
            exporter.initialize().unwrap();
            exporter.finalize().unwrap();
            exporter
                .write_template_stats(&[make_template_stats_sqlite("B")], None)
                .unwrap();
        }

        let conn = rusqlite::Connection::open(&dbfile).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sql_templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 2,
            "append should retain old row + add new, got {count}"
        );

        let keys: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT template_key FROM sql_templates ORDER BY template_key")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert_eq!(keys, vec!["A", "B"], "expected keys [A, B], got {keys:?}");
    }
}

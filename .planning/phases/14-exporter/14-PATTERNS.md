# Phase 14: Exporter 集成输出 - Pattern Map

**Mapped:** 2026-05-16
**Files analyzed:** 4 (new/modified)
**Analogs found:** 4 / 4

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/exporter/mod.rs` | trait-def + dispatcher | request-response | `src/exporter/mod.rs` (现有 trait 方法扩展) | exact (self) |
| `src/exporter/sqlite.rs` | storage/exporter | CRUD | `src/exporter/sqlite.rs` (现有 finalize/initialize 模式) | exact (self) |
| `src/exporter/csv.rs` | file-I/O/exporter | file-I/O | `src/exporter/csv.rs` (现有 initialize 文件写入) | exact (self) |
| `src/cli/run.rs` | orchestration | request-response | `src/cli/run.rs` (现有 finalize 调用点) | exact (self) |

---

## Pattern Assignments

### `src/exporter/mod.rs` — Exporter trait 新增第四段 + ExporterKind 透传 + ExporterManager 外部接口

**修改点 1：Exporter trait 新增默认方法**

参照现有 `export_one_preparsed` 的默认实现模式（`mod.rs` L28-39）：提供默认 no-op 实现，让未来 exporter 可以选择不实现。

**Imports pattern** (`mod.rs` L1-10，现有，无需新增):
```rust
use crate::config::Config;
use crate::error::{ConfigError, Error, Result};
use dm_database_parser_sqllog::{MetaParts, PerformanceMetrics, Sqllog};
use log::info;
```

**现有 trait 默认方法模式**（`mod.rs` L18-39，直接复制结构）:
```rust
// 新方法跟随此结构加入 trait
fn export_one_normalized(
    &mut self,
    sqllog: &Sqllog<'_>,
    normalized: Option<&str>,
) -> Result<()> {
    let _ = normalized;          // ← 忽略额外参数的惯用写法
    self.export(sqllog)
}
```

**新增 trait 方法（照此模式）**:
```rust
fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    final_path: Option<&std::path::Path>,
) -> Result<()> {
    let _ = (stats, final_path);   // 与上方 `let _ = normalized` 完全一致
    Ok(())
}
```

**修改点 2：ExporterKind 透传 arm**

参照现有 `finalize()` 透传（`mod.rs` L99-105）：

```rust
fn finalize(&mut self) -> Result<()> {
    match self {
        Self::Csv(e) => e.finalize(),
        Self::Sqlite(e) => e.finalize(),
        Self::DryRun(e) => e.finalize(),
    }
}
```

**新增透传（照此模式，在 ExporterKind impl 块内）**:
```rust
fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    final_path: Option<&std::path::Path>,
) -> Result<()> {
    match self {
        Self::Csv(e) => e.write_template_stats(stats, final_path),
        Self::Sqlite(e) => e.write_template_stats(stats, final_path),
        Self::DryRun(e) => e.write_template_stats(stats, final_path),
    }
}
```

**修改点 3：ExporterManager 外部方法**

参照现有 `finalize()` 外部方法（`mod.rs` L274-279）：

```rust
pub fn finalize(&mut self) -> Result<()> {
    info!("Finalizing exporters...");
    self.exporter.finalize()?;
    info!("Exporters finished");
    Ok(())
}
```

**新增外部方法（照此模式，不加 info! wrapper，直接委托）**:
```rust
pub fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    final_path: Option<&std::path::Path>,
) -> Result<()> {
    self.exporter.write_template_stats(stats, final_path)
}
```

**DryRunExporter 覆盖（D-05：只打 info!，不产生文件）**

参照 `DryRunExporter::export_one_preparsed`（`mod.rs` L158-169）：直接计数/记录，无文件 I/O。

```rust
// DryRunExporter 现有 finalize: 直接 Ok(())
fn finalize(&mut self) -> Result<()> {
    Ok(())
}
```

**新增 DryRunExporter::write_template_stats 覆盖**:
```rust
fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    _final_path: Option<&std::path::Path>,
) -> Result<()> {
    info!(
        "Dry-run: would write {} template stats (no file written)",
        stats.len()
    );
    Ok(())
}
```

---

### `src/exporter/sqlite.rs` — SqliteExporter::write_template_stats()

**Analog:** 同文件的 `finalize()`（L397-407）+ `prepare_target_table()`（L239-255）+ `initialize()`（L280-312）

**借用安全模式**（Pitfall 1 预防，参照 `batch_commit_if_needed` L137-145）：

先 copy 标志字段到局部变量，再借用 `self.conn`，避免部分借用冲突：

```rust
fn batch_commit_if_needed(&mut self) -> Result<()> {
    self.row_count += 1;
    if self.row_count % self.batch_size == 0 {
        let conn = self.conn.as_ref().unwrap();   // ← 借用在 row_count 已用完之后
        conn.execute_batch("COMMIT; BEGIN")
            .map_err(|e| Self::db_err(format!("batch commit failed: {e}")))?;
    }
    Ok(())
}
```

**finalize() 不关闭连接模式**（D-06 依赖，`sqlite.rs` L397-407）：

```rust
fn finalize(&mut self) -> Result<()> {
    if let Some(conn) = &self.conn {    // ← &self.conn，不是 self.conn.take()
        conn.execute_batch("COMMIT;")
            .map_err(|e| Self::db_err(format!("commit failed: {e}")))?;
    }
    info!(
        "SQLite export finished: {} (success: {}, failed: {})",
        self.database_url, self.stats.exported, self.stats.failed
    );
    Ok(())
}
```

**DDL 构建模式**（参照 `build_create_sql` L88-115）：固定字面量 DDL，不走动态列生成。

`prepare_target_table` 的 overwrite 分支（L239-254）：

```rust
fn prepare_target_table(&self) -> Result<()> {
    if self.overwrite {
        let conn = self.conn.as_ref().unwrap();
        conn.execute(&format!("DROP TABLE IF EXISTS \"{}\"", self.table_name), [])
            .map_err(|e| Self::db_err(format!("drop table failed: {e}")))?;
        info!("Dropped existing table: {}", self.table_name);
    } else if !self.append {
        // ...
    }
    Ok(())
}
```

**error 映射模式**（`db_err` L129-133）：

```rust
fn db_err(reason: impl Into<String>) -> Error {
    Error::Export(ExportError::DatabaseFailed {
        reason: reason.into(),
    })
}
```

**write_template_stats 完整实现（组合以上模式）**:

```rust
fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    _final_path: Option<&std::path::Path>,
) -> Result<()> {
    // 先读标志，再借用 conn（Pitfall 1 预防）
    let overwrite = self.overwrite;
    let conn = self.conn.as_ref()
        .ok_or_else(|| Self::db_err("write_template_stats: not initialized"))?;

    // D-07/D-08: overwrite → DROP IF EXISTS；append → CREATE IF NOT EXISTS
    if overwrite {
        conn.execute("DROP TABLE IF EXISTS sql_templates", [])
            .map_err(|e| Self::db_err(format!("drop sql_templates failed: {e}")))?;
    }
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sql_templates (
            template_key TEXT NOT NULL PRIMARY KEY,
            count        INTEGER NOT NULL,
            avg_us       INTEGER NOT NULL,
            min_us       INTEGER NOT NULL,
            max_us       INTEGER NOT NULL,
            p50_us       INTEGER NOT NULL,
            p95_us       INTEGER NOT NULL,
            p99_us       INTEGER NOT NULL,
            first_seen   TEXT NOT NULL,
            last_seen    TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| Self::db_err(format!("create sql_templates failed: {e}")))?;

    // 单事务批量 INSERT（数量远小于主记录，无需分批）
    conn.execute_batch("BEGIN;")
        .map_err(|e| Self::db_err(format!("begin sql_templates tx failed: {e}")))?;
    for s in stats {
        conn.execute(
            "INSERT INTO sql_templates VALUES (?,?,?,?,?,?,?,?,?,?)",
            rusqlite::params![
                s.template_key,
                s.count,
                s.avg_us,
                s.min_us,
                s.max_us,
                s.p50_us,
                s.p95_us,
                s.p99_us,
                s.first_seen,
                s.last_seen
            ],
        )
        .map_err(|e| Self::db_err(format!("insert sql_templates failed: {e}")))?;
    }
    conn.execute_batch("COMMIT;")
        .map_err(|e| Self::db_err(format!("commit sql_templates failed: {e}")))?;

    info!("sql_templates: {} rows written to {}", stats.len(), self.database_url);
    Ok(())
}
```

**测试模式**（参照 `test_sqlite_basic_export` L434-463）：用作用域确保 exporter drop 后再开新连接验证（EXCLUSIVE lock 释放）：

```rust
#[test]
fn test_sqlite_write_template_stats() {
    let dir = tempfile::TempDir::new().unwrap();
    let dbfile = dir.path().join("out.db");

    {
        let mut exporter = SqliteExporter::new(
            dbfile.to_string_lossy().into(), "sqllog_records".into(), true, false,
        );
        exporter.initialize().unwrap();
        exporter.finalize().unwrap();

        let stats = vec![/* ... */];
        exporter.write_template_stats(&stats, None).unwrap();
        // exporter drop 在此处 → EXCLUSIVE lock 释放
    }

    // 再开连接验证
    let conn = rusqlite::Connection::open(&dbfile).unwrap();
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM sql_templates", [], |r| r.get(0)).unwrap();
    assert_eq!(count, /* expected */);
}
```

---

### `src/exporter/csv.rs` — CsvExporter::write_template_stats()

**Analog:** 同文件的 `initialize()`（L347-391）+ `write_csv_escaped()`（L13-21）

**文件创建模式**（参照 `initialize()` L347-391）：

```rust
fn initialize(&mut self) -> Result<()> {
    ensure_parent_dir(&self.path).map_err(|e| {
        Error::Export(ExportError::WriteFailed {
            path: self.path.clone(),
            reason: format!("create dir failed: {e}"),
        })
    })?;

    // ... OpenOptions ...
    let file = /* ... */
        .map_err(|e| {
            Error::Export(ExportError::WriteFailed {
                path: self.path.clone(),
                reason: format!("open failed: {e}"),
            })
        })?;

    let mut writer = BufWriter::with_capacity(16 * 1024 * 1024, file);
    // ... write header ...
    self.writer = Some(writer);
    Ok(())
}
```

**write_template_stats 中简化的文件写入**：伴随文件不需要 BufWriter 大缓冲（数据量小），使用 `std::fs::File::create`（始终覆盖，D-10）：

```rust
fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    final_path: Option<&std::path::Path>,
) -> Result<()> {
    // D-09: 路径推导
    let base_path = final_path.unwrap_or(&self.path);
    let stem = base_path.file_stem().unwrap_or_default();
    let companion = base_path.with_file_name(
        format!("{}_templates.csv", stem.to_string_lossy())
    );

    // D-10: 始终覆盖（ensure_parent_dir 是 pub(super)，csv.rs 属于 super 模块内）
    ensure_parent_dir(&companion).map_err(|e| {
        Error::Export(ExportError::WriteFailed {
            path: companion.clone(),
            reason: format!("create dir failed: {e}"),
        })
    })?;

    let file = std::fs::File::create(&companion).map_err(|e| {
        Error::Export(ExportError::WriteFailed {
            path: companion.clone(),
            reason: format!("create companion failed: {e}"),
        })
    })?;
    let mut writer = std::io::BufWriter::new(file);

    // 写固定表头（与 sql_templates 列名一致）
    writer.write_all(
        b"template_key,count,avg_us,min_us,max_us,p50_us,p95_us,p99_us,first_seen,last_seen\n"
    ).map_err(|e| Error::Export(ExportError::WriteFailed {
        path: companion.clone(),
        reason: format!("write header failed: {e}"),
    }))?;

    // 写数据行：template_key 用 write_csv_escaped，数值用 itoa
    let mut itoa_buf = itoa::Buffer::new();
    let mut line_buf: Vec<u8> = Vec::with_capacity(512);
    for s in stats {
        line_buf.clear();
        // template_key 可能含 '"' 或 ','，必须 CSV 转义包裹
        line_buf.push(b'"');
        write_csv_escaped(&mut line_buf, s.template_key.as_bytes());
        line_buf.push(b'"');
        line_buf.push(b',');
        line_buf.extend_from_slice(itoa_buf.format(s.count).as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(itoa_buf.format(s.avg_us).as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(itoa_buf.format(s.min_us).as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(itoa_buf.format(s.max_us).as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(itoa_buf.format(s.p50_us).as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(itoa_buf.format(s.p95_us).as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(itoa_buf.format(s.p99_us).as_bytes());
        line_buf.push(b',');
        // first_seen/last_seen 是普通时间字符串，通常不含特殊字符，直接写
        line_buf.extend_from_slice(s.first_seen.as_bytes());
        line_buf.push(b',');
        line_buf.extend_from_slice(s.last_seen.as_bytes());
        line_buf.push(b'\n');
        writer.write_all(&line_buf).map_err(|e| Error::Export(ExportError::WriteFailed {
            path: companion.clone(),
            reason: format!("write row failed: {e}"),
        }))?;
    }

    writer.flush().map_err(|e| Error::Export(ExportError::WriteFailed {
        path: companion.clone(),
        reason: format!("flush failed: {e}"),
    }))?;

    info!("Template companion CSV written: {}", companion.display());
    Ok(())
}
```

**关键：** `write_csv_escaped` 是同模块级私有函数（`csv.rs` L13），`write_template_stats` 在同文件实现，可直接调用，无需改可见性。

---

### `src/cli/run.rs` — 两处插入 write_template_stats 调用

**顺序路径插入点**（`run.rs` L886-895，已验证）：

```rust
// 现有代码（L886-895）
exporter_manager.finalize()?;
if !quiet {
    exporter_manager.log_stats();
}

// Phase 14 将消费 finalize() 结果并写出报告；此处先记录聚合摘要。
let template_stats = template_agg.map(TemplateAggregator::finalize);
if let Some(ref stats) = template_stats {
    info!("Template analysis: {} unique templates", stats.len());
    // ← 在此插入：
    // exporter_manager.write_template_stats(stats, None)?;
}
```

**插入后（顺序路径）**:
```rust
let template_stats = template_agg.map(TemplateAggregator::finalize);
if let Some(ref stats) = template_stats {
    info!("Template analysis: {} unique templates", stats.len());
    exporter_manager.write_template_stats(stats, None)?;
}
```

**并行路径插入点**（`run.rs` L791-795，已验证）：

```rust
// 现有代码（L791-795）
let template_stats = parallel_agg.map(TemplateAggregator::finalize);
if let Some(ref stats) = template_stats {
    info!("Template analysis: {} unique templates", stats.len());
    // ← 在此插入调用，但此处没有 ExporterManager 实例
}
```

**并行路径解法**（D-02/D-03，基于 `ExporterManager::from_csv` 已有构造器，`mod.rs` L196-199）：

```rust
// run.rs 现有 from_csv 构造器（mod.rs L196-199）
pub fn from_csv(exporter: CsvExporter) -> Self {
    Self {
        exporter: ExporterKind::Csv(exporter),
    }
}
```

**插入后（并行路径）**:
```rust
let template_stats = parallel_agg.map(TemplateAggregator::finalize);
if let Some(ref stats) = template_stats {
    info!("Template analysis: {} unique templates", stats.len());
    // 并行路径无公共 ExporterManager；构造临时实例仅用于写伴随文件
    // csv_cfg.file 是 concat_csv_parts 完成后的最终输出路径（D-03）
    if let Some(csv_cfg) = &final_cfg.exporter.csv {
        let tmp_csv = CsvExporter::new(&csv_cfg.file);
        let mut tmp_em = ExporterManager::from_csv(tmp_csv);
        tmp_em.write_template_stats(stats, Some(std::path::Path::new(&csv_cfg.file)))?;
    }
}
```

**所需新 use 路径**：`run.rs` L5 已有 `use crate::exporter::{CsvExporter, ExporterManager}`，无需新增。`std::path::Path` 已在其他地方间接可用，或直接用 `Path::new`（`use std::path::Path` L17 已有）。

---

## Shared Patterns

### Error 映射模式（SQLite）
**Source:** `src/exporter/sqlite.rs` L129-133
**Apply to:** `SqliteExporter::write_template_stats`

```rust
fn db_err(reason: impl Into<String>) -> Error {
    Error::Export(ExportError::DatabaseFailed {
        reason: reason.into(),
    })
}
// 使用：.map_err(|e| Self::db_err(format!("操作失败: {e}")))?
```

### Error 映射模式（CSV 文件 I/O）
**Source:** `src/exporter/csv.rs` L347-353
**Apply to:** `CsvExporter::write_template_stats`

```rust
// 统一错误包装格式
Error::Export(ExportError::WriteFailed {
    path: companion.clone(),
    reason: format!("操作失败: {e}"),
})
```

### 目录创建辅助
**Source:** `src/exporter/mod.rs` L353-358
**Apply to:** `CsvExporter::write_template_stats`（写伴随文件前）

```rust
pub(super) fn ensure_parent_dir(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.exists()) {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
```

### CSV 转义
**Source:** `src/exporter/csv.rs` L13-21
**Apply to:** `CsvExporter::write_template_stats` 的 template_key 字段

```rust
fn write_csv_escaped(buf: &mut Vec<u8>, bytes: &[u8]) {
    let mut remaining = bytes;
    while let Some(pos) = memchr::memchr(b'"', remaining) {
        buf.extend_from_slice(&remaining[..=pos]);
        buf.push(b'"');
        remaining = &remaining[pos + 1..];
    }
    buf.extend_from_slice(remaining);
}
// 调用方式：line_buf.push(b'"'); write_csv_escaped(&mut line_buf, s.template_key.as_bytes()); line_buf.push(b'"');
```

### SQLite EXCLUSIVE lock 测试规避
**Source:** `src/exporter/sqlite.rs` L434-463（test_sqlite_basic_export）
**Apply to:** 所有 `SqliteExporter::write_template_stats` 相关测试

```rust
{
    let mut exporter = SqliteExporter::new(...);
    exporter.initialize().unwrap();
    // ... 写入 ...
    exporter.finalize().unwrap();
    exporter.write_template_stats(&stats, None).unwrap();
    // exporter 在此 drop → EXCLUSIVE lock 释放
}
// 此作用域外才开新连接验证
let conn = rusqlite::Connection::open(&dbfile).unwrap();
```

### 整数序列化（零分配）
**Source:** `src/exporter/csv.rs` L30（`itoa_buf: itoa::Buffer`）
**Apply to:** `CsvExporter::write_template_stats` 的数值字段写入

```rust
let mut itoa_buf = itoa::Buffer::new();
// 使用：itoa_buf.format(s.count).as_bytes()
```

---

## No Analog Found

本 Phase 无无类比文件。所有修改点在同项目内均有直接对应的现有模式。

---

## TemplateStats 字段参考

**Source:** `src/features/template_aggregator.rs` L26-37

```rust
pub struct TemplateStats {
    pub template_key: String,  // SQL 模板（normalize 后）
    pub count: u64,
    pub avg_us: u64,           // 所有字段单位均为微秒
    pub min_us: u64,
    pub max_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub first_seen: String,    // 时间戳字符串
    pub last_seen: String,
}
```

**sql_templates DDL 列顺序**（与 ROADMAP 一致）：
`template_key, count, avg_us, min_us, max_us, p50_us, p95_us, p99_us, first_seen, last_seen`

---

## Metadata

**Analog search scope:** `src/exporter/`, `src/cli/run.rs`, `src/features/template_aggregator.rs`
**Files scanned:** 5
**Pattern extraction date:** 2026-05-16

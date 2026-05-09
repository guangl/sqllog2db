# Phase 5: SQLite 性能优化 - Pattern Map

**Mapped:** 2026-05-09
**Files analyzed:** 4 (2 modified source files + 1 modified bench file + 1 doc update)
**Analogs found:** 4 / 4

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/exporter/sqlite.rs` | exporter/service | CRUD (batch write) | `src/exporter/sqlite.rs` (self, current state) | exact — direct modification |
| `src/config.rs` | config | — | `src/config.rs` `CsvExporter` struct (lines 305–340) | exact — same file, same pattern |
| `benches/bench_sqlite.rs` | benchmark | batch I/O | `benches/bench_sqlite.rs` `bench_sqlite_export` (lines 62–95) | exact — same file, extend pattern |
| `benches/BENCHMARKS.md` | doc | — | `benches/BENCHMARKS.md` Phase 4 section (lines 128–200) | exact — append same table format |

---

## Pattern Assignments

### `src/exporter/sqlite.rs` (exporter, CRUD batch write)

**Analog:** 自身当前代码，直接修改。

---

#### 1. 结构体新增字段

**参考：** `src/exporter/sqlite.rs` lines 9–20（SqliteExporter struct）

当前结构体：
```rust
pub struct SqliteExporter {
    database_url: String,
    table_name: String,
    insert_sql: String,
    overwrite: bool,
    append: bool,
    conn: Option<Connection>,
    stats: ExportStats,
    pub(super) normalize: bool,
    pub(super) field_mask: crate::features::FieldMask,
    pub(super) ordered_indices: Vec<usize>,
}
```

Phase 5 需新增两个字段，紧跟在 `stats` 之后：
```rust
    row_count: usize,    // 当前批次已写行数，达 batch_size 时触发 COMMIT+BEGIN
    batch_size: usize,   // 每批提交行数，来自 config，默认 10_000
```

`new()` 和 `from_config()` 同步更新（见下方）。

---

#### 2. `new()` 和 `from_config()` 更新

**参考：** `src/exporter/sqlite.rs` lines 34–107

`new()` 新增参数（或保持原签名，用 `batch_size` 独立 setter）：
```rust
// 选项 A：`new()` 保持不变，from_config 设置字段（推荐，避免破坏 13 个测试的调用点）
impl SqliteExporter {
    pub fn new(database_url: String, table_name: String, overwrite: bool, append: bool) -> Self {
        // ...现有逻辑...
        Self {
            // ...现有字段...
            row_count: 0,
            batch_size: 10_000,   // 合理默认值，测试时可通过字段直接改写
        }
    }

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
}
```

---

#### 3. `initialize()` — PRAGMA 顺序修正 + WAL 验证

**参考：** `src/exporter/sqlite.rs` lines 205–254（当前 `initialize()` 实现）

**当前问题代码（lines 217–226）：**
```rust
conn.execute_batch(
    "PRAGMA journal_mode = OFF;    // ← 改为 WAL
     PRAGMA synchronous = OFF;
     PRAGMA cache_size = 1000000;
     PRAGMA locking_mode = EXCLUSIVE;
     PRAGMA temp_store = MEMORY;
     PRAGMA mmap_size = 30000000000;
     PRAGMA page_size = 65536;    // ← Bug! 必须在 WAL 之前！
     PRAGMA threads = 4;",
)
```

**Phase 5 替换为两步调用（提取 `initialize_pragmas()` 辅助函数，保持 `initialize()` ≤40 行）：**
```rust
fn initialize_pragmas(conn: &Connection) -> std::result::Result<(), rusqlite::Error> {
    // 必须第一个：page_size 须在 journal_mode=WAL 之前生效
    conn.execute_batch("PRAGMA page_size = 65536;")?;

    // 验证 WAL 模式已启用（原子设置+验证，标准 API）
    let journal_mode: String = conn.pragma_update_and_check(
        None,
        "journal_mode",
        "WAL",
        |row| row.get(0),
    )?;
    if journal_mode != "wal" {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            Some(format!("journal_mode=WAL not supported, got: {journal_mode}")),
        ));
    }

    conn.execute_batch(
        "PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = 1000000;
         PRAGMA locking_mode = EXCLUSIVE;
         PRAGMA temp_store = MEMORY;
         PRAGMA mmap_size = 30000000000;
         PRAGMA threads = 4;",
    )?;
    Ok(())
}
```

> 注：错误返回应走 `Self::db_err()` 包装路径，在 `initialize()` 中用 `.map_err(|e| Self::db_err(...))?` 调用 `initialize_pragmas(conn)`。

`initialize()` 中替换原 `conn.execute_batch(...)` 调用块：
```rust
initialize_pragmas(&conn)
    .map_err(|e| Self::db_err(format!("set PRAGMAs failed: {e}")))?;
```

**完整 `initialize()` 结构（确保函数体 ≤40 行）：**
```rust
fn initialize(&mut self) -> Result<()> {
    info!("Initializing SQLite exporter: {}", self.database_url);

    let path = Path::new(&self.database_url);
    if let Some(parent) = path.parent().filter(|p| !p.exists()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| Self::db_err(format!("create dir failed: {e}")))?;
    }

    let conn = Connection::open(&self.database_url)
        .map_err(|e| Self::db_err(format!("open failed: {e}")))?;

    initialize_pragmas(&conn)
        .map_err(|e| Self::db_err(format!("set PRAGMAs failed: {e}")))?;

    self.conn = Some(conn);
    self.row_count = 0;   // 重置批次计数器（支持重复调用 initialize）

    // ...现有的 overwrite / append / create table 逻辑（lines 231–247 不变）...

    let conn = self.conn.as_ref().unwrap();
    conn.execute_batch("BEGIN TRANSACTION;")
        .map_err(|e| Self::db_err(format!("begin transaction failed: {e}")))?;

    info!("SQLite exporter initialized: {}", self.database_url);
    Ok(())
}
```

---

#### 4. `export_one_preparsed()` — 批量提交逻辑

**参考：** `src/exporter/sqlite.rs` lines 301–328（当前 `export_one_preparsed()`）

当前热路径末尾（lines 322–327）：
```rust
        .map_err(|e| Self::db_err(format!("insert failed: {e}")))?;
        self.stats.record_success();
        Ok(())
```

Phase 5 在 `record_success()` 之后插入批量提交逻辑（提取辅助函数 `batch_commit_if_needed()` 保持热路径 ≤40 行）：
```rust
fn batch_commit_if_needed(&mut self) -> Result<()> {
    self.row_count += 1;
    if self.row_count % self.batch_size == 0 {
        let conn = self.conn.as_ref().unwrap();
        conn.execute_batch("COMMIT; BEGIN")
            .map_err(|e| Self::db_err(format!("batch commit failed: {e}")))?;
        self.stats.flush_operations += 1;
        self.stats.last_flush_size = self.batch_size;
    }
    Ok(())
}
```

在三个 export 方法（`export()`、`export_one_normalized()`、`export_one_preparsed()`）的 `self.stats.record_success()` 之后调用：
```rust
        self.stats.record_success();
        self.batch_commit_if_needed()?;
        Ok(())
```

---

#### 5. `finalize()` — WAL checkpoint（可选）

**参考：** `src/exporter/sqlite.rs` lines 330–345（当前 `finalize()`）

当前 `finalize()` 只做 COMMIT：
```rust
fn finalize(&mut self) -> Result<()> {
    if let Some(conn) = &self.conn {
        conn.execute_batch("COMMIT;")
            .map_err(|e| Self::db_err(format!("commit failed: {e}")))?;
    }
    // ...info! log...
    Ok(())
}
```

Phase 5 在 COMMIT 后追加 WAL checkpoint（TRUNCATE 模式确保不遗留大 WAL 文件）：
```rust
        conn.execute_batch("COMMIT;")
            .map_err(|e| Self::db_err(format!("commit failed: {e}")))?;
        // WAL checkpoint：将 WAL 合并到主文件，避免遗留 .db-wal 文件
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| Self::db_err(format!("wal checkpoint failed: {e}")))?;
```

---

#### 6. 新增集成测试（tests 模块中）

**参考：** `src/exporter/sqlite.rs` lines 347–396（`write_test_log` + `test_sqlite_basic_export` 模式）

所有现有测试共用 `{}` 块确保 exporter drop 后再打开验证连接，Phase 5 新测试必须沿用此模式：

```rust
// Pattern: exporter 在 {} 块中创建和销毁，释放 EXCLUSIVE lock
{
    let mut exporter = SqliteExporter::new(...);
    exporter.initialize().unwrap();
    // ... insert records ...
    exporter.finalize().unwrap();
} // exporter drops here → EXCLUSIVE lock released

// 验证代码在 {} 块外打开同一 .db
let conn = rusqlite::Connection::open(&dbfile).unwrap();
```

**三个需新增的测试：**

```rust
// 测试1: PERF-05 — WAL 模式启用断言
#[test]
fn test_sqlite_wal_mode_enabled() { ... }

// 测试2: page_size Bug 修正断言
#[test]
fn test_sqlite_wal_page_size() { ... }

// 测试3: PERF-04 — 批量提交路径覆盖（batch_size=2，写 5 条，断言中间 COMMIT 触发）
#[test]
fn test_sqlite_batch_commit() {
    // 关键：设 exporter.batch_size = 2（直接赋字段，new() 后覆盖默认值）
    // 写 5 条，触发 2 次中间 COMMIT（第2条、第4条后），最终 finalize() COMMIT 第5条
    // 验证：打开 db 读取 COUNT(*) == 5（所有中间 COMMIT 均已持久化）
}
```

---

### `src/config.rs` (config, —)

**Analog:** 同文件 `CsvExporter` struct（lines 305–340）——serde default 函数 + `Default` impl 的标准模式。

---

#### 1. `SqliteExporter` struct 新增 `batch_size` 字段

**参考：** `src/config.rs` lines 306–316（`CsvExporter` 字段的 serde default 用法）

```rust
// CsvExporter 的参考模式（lines 306–316）：
pub struct CsvExporter {
    pub file: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    #[serde(default)]
    pub append: bool,
    #[serde(default = "default_true")]
    pub include_performance_metrics: bool,
}
```

对应 `SqliteExporter` 新增字段（lines 342–351，在 `append` 之后）：
```rust
pub struct SqliteExporter {
    pub database_url: String,
    #[serde(default = "default_table_name")]
    pub table_name: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    #[serde(default)]
    pub append: bool,
    #[serde(default = "default_sqlite_batch_size")]
    pub batch_size: usize,   // 新增：每批提交行数，默认 10_000
}

fn default_sqlite_batch_size() -> usize {
    10_000
}
```

---

#### 2. `Default` impl 更新

**参考：** `src/config.rs` lines 357–366（`SqliteExporter` 的 `Default` impl）

```rust
// 当前 Default（lines 357–366）：
impl Default for SqliteExporter {
    fn default() -> Self {
        Self {
            database_url: "export/sqllog2db.db".to_string(),
            table_name: "sqllog_records".to_string(),
            overwrite: true,
            append: false,
        }
    }
}

// Phase 5 更新后：
impl Default for SqliteExporter {
    fn default() -> Self {
        Self {
            database_url: "export/sqllog2db.db".to_string(),
            table_name: "sqllog_records".to_string(),
            overwrite: true,
            append: false,
            batch_size: 10_000,   // 新增
        }
    }
}
```

---

#### 3. `apply_one()` 新增 `batch_size` 覆盖项

**参考：** `src/config.rs` lines 152–175（`exporter.sqlite.*` 的 apply_one 模式）

在 `"exporter.sqlite.append"` 分支之后追加：
```rust
"exporter.sqlite.batch_size" => {
    self.exporter
        .sqlite
        .get_or_insert_with(Default::default)
        .batch_size = value.parse().map_err(|_| {
        Error::Config(ConfigError::InvalidValue {
            field: key.to_string(),
            value: value.to_string(),
            reason: "expected a positive integer".to_string(),
        })
    })?;
}
```

---

#### 4. 现有测试对 `SqliteExporter` 结构体字面量的影响

**参考：** `src/config.rs` lines 424–428、491–501、648–654

测试中直接构造 `SqliteExporter { ... }` 的地方（结构体字面量），新增字段后会导致编译报错。**修复方式：** 在所有测试的结构体字面量中补全 `batch_size: 10_000`，或改用 `..SqliteExporter::default()`。

`src/exporter/mod.rs` 中 test（lines 491–501）同样需要补全：
```rust
// 修复前（lines 493–500）：
sqlite: Some(SqliteExporterCfg {
    database_url: "/tmp/test_mod.db".to_string(),
    table_name: "records".to_string(),
    overwrite: true,
    append: false,
}),
// 修复后：
sqlite: Some(SqliteExporterCfg {
    database_url: "/tmp/test_mod.db".to_string(),
    table_name: "records".to_string(),
    overwrite: true,
    append: false,
    batch_size: 10_000,
}),
```

---

### `benches/bench_sqlite.rs` (benchmark, batch I/O)

**Analog:** 自身 `bench_sqlite_export` 函数（lines 62–95），直接扩展。

---

#### 1. `make_config()` 新增 `batch_size` 参数

**参考：** `benches/bench_sqlite.rs` lines 33–60（`make_config()` 函数）

当前 `make_config()` TOML 模板中 `[exporter.sqlite]` 部分（lines 50–55）：
```rust
[exporter.sqlite]
database_url = "{dir}/bench.db"
table_name = "sqllogs"
overwrite = true
append = false
```

Phase 5 新增 `batch_size` 参数（函数签名同步更新）：
```rust
fn make_config(sqllog_dir: &Path, bench_dir: &Path, batch_size: usize) -> Config {
    let toml = format!(
        r#"
...
[exporter.sqlite]
database_url = "{dir}/bench.db"
table_name = "sqllogs"
overwrite = true
append = false
batch_size = {batch_size}
"#,
        // ...
        batch_size = batch_size,
    );
    toml::from_str(&toml).unwrap()
}
```

现有 `bench_sqlite_export` 和 `bench_sqlite_real_file` 调用点改为 `make_config(..., 10_000)`。

---

#### 2. 新增 `bench_sqlite_single_row` 函数

**参考：** `benches/bench_sqlite.rs` lines 62–95（`bench_sqlite_export` 的完整模式）

```rust
fn bench_sqlite_single_row(c: &mut Criterion) {
    let bench_dir = PathBuf::from("target/bench_sqlite_single_row");
    let sqllog_dir = bench_dir.join("sqllogs");
    fs::create_dir_all(&sqllog_dir).unwrap();

    let mut group = c.benchmark_group("sqlite_single_row");
    // 单行提交极慢（每次 fsync），大幅减少采样次数
    group.sample_size(10);

    // 对比意义在 10_000+ 条时才显著；不需要 50_000（太慢）
    for &n in &[1_000usize, 10_000] {
        fs::write(sqllog_dir.join("bench.log"), synthetic_log(n)).unwrap();
        // batch_size=1：每条 INSERT 独立 BEGIN/COMMIT（单行提交模式）
        let cfg = make_config(&sqllog_dir, &bench_dir, 1);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &cfg, |b, cfg| {
            b.iter(|| {
                handle_run(
                    cfg,
                    None,
                    false,
                    true,
                    &Arc::new(AtomicBool::new(false)),
                    80,
                    false,
                    None,
                    1,
                )
                .unwrap();
            });
        });
    }

    group.finish();
}
```

---

#### 3. `criterion_group!` 宏更新

**参考：** `benches/bench_sqlite.rs` line 133

当前：
```rust
criterion_group!(benches, bench_sqlite_export, bench_sqlite_real_file);
```

Phase 5 更新后：
```rust
criterion_group!(benches, bench_sqlite_export, bench_sqlite_single_row, bench_sqlite_real_file);
```

---

### `benches/BENCHMARKS.md` (doc, —)

**Analog:** 同文件 Phase 4 section（lines 128–200），追加相同格式的 Phase 5 section。

Phase 5 section 结构模板（在 Phase 4 section 末尾之后追加）：

```markdown
---

## Phase 5 — SQLite 性能优化（v1.2）

**Date:** 2026-05-09
**Goal:** WAL 模式 + 批量事务，sqlite_export/10000 ≤ 7.424ms hard limit

### 各 Wave 数值

| Group | v1.0 baseline | Phase 5 实测 | vs v1.0 |
|-------|--------------|--------------|---------|
| sqlite_export/1000    | 0.851 ms | TBD | TBD |
| sqlite_export/10000   | 7.070 ms | TBD | TBD |
| sqlite_export/50000   | 35.603 ms | TBD | TBD |
| sqlite_single_row/1000 | — | TBD | — |
| sqlite_single_row/10000 | — | TBD | — |

### Criterion 输出原文

（Phase 5 执行后补充）

### 解读

（Phase 5 执行后补充）

### 结论

- [ ] PERF-04 批量事务 benchmark 可量化
- [ ] PERF-05 WAL 模式已启用（测试通过）
- [ ] PERF-06 prepared statement 复用确认（代码审查 + flamegraph）
- [ ] sqlite_export/10000 ≤ 7.424ms hard limit
```

---

## Shared Patterns

### 错误处理模式
**Source:** `src/exporter/sqlite.rs` lines 109–113 (`db_err()`) + lines 204–227（`initialize()` 中 `.map_err(|e| Self::db_err(...))` 链）
**Apply to:** `initialize_pragmas()`（调用处包装）、`batch_commit_if_needed()`、`finalize()` 新增 checkpoint
```rust
fn db_err(reason: impl Into<String>) -> Error {
    Error::Export(ExportError::DatabaseFailed {
        reason: reason.into(),
    })
}

// 使用模式：所有 rusqlite 错误通过此函数包装
some_rusqlite_call()
    .map_err(|e| Self::db_err(format!("context: {e}")))?;
```

### serde 默认值函数模式
**Source:** `src/config.rs` lines 222–229（`default_logging_file`、`default_logging_level`）及 lines 388–390（`default_true()`）
**Apply to:** `config.rs` 新增的 `default_sqlite_batch_size()`
```rust
// 命名约定：default_{字段名}()
fn default_sqlite_batch_size() -> usize {
    10_000
}

// 在字段上引用：
#[serde(default = "default_sqlite_batch_size")]
pub batch_size: usize,
```

### 测试中 EXCLUSIVE lock 释放模式
**Source:** `src/exporter/sqlite.rs` lines 375–396（`test_sqlite_basic_export`）
**Apply to:** 三个新集成测试（`test_sqlite_wal_mode_enabled`、`test_sqlite_wal_page_size`、`test_sqlite_batch_commit`）
```rust
// exporter 在 {} 块中 drop → EXCLUSIVE lock released
{
    let mut exporter = SqliteExporter::new(...);
    exporter.initialize().unwrap();
    // insert records
    exporter.finalize().unwrap();
} // drop here

// 验证连接在 {} 块外打开
let conn = rusqlite::Connection::open(&dbfile).unwrap();
let count: i64 = conn.query_row(...).unwrap();
```

### `pragma_update_and_check` 验证模式
**Source:** RESEARCH.md Pattern 1（rusqlite pragma.rs VERIFIED）
**Apply to:** `initialize_pragmas()` 中验证 WAL 启用；测试中断言 journal_mode
```rust
let journal_mode: String = conn.pragma_update_and_check(
    None,
    "journal_mode",
    "WAL",
    |row| row.get(0),
)?;
// 返回值已规范化为小写："wal"
assert_eq!(journal_mode, "wal");
```

### benchmark `handle_run` 调用模式
**Source:** `benches/bench_sqlite.rs` lines 76–91（`bench_sqlite_export` 中的 `b.iter(...)` 块）
**Apply to:** `bench_sqlite_single_row` 函数中的 `b.iter(...)` 块（参数完全相同）
```rust
b.iter(|| {
    handle_run(
        cfg,
        None,
        false,
        true,  // quiet=true：排除进度条 I/O 对吞吐量测量的干扰
        &Arc::new(AtomicBool::new(false)),
        80,
        false,
        None,
        1,
    )
    .unwrap();
});
```

---

## No Analog Found

所有文件均有明确 analog，无需回退到 RESEARCH.md 独立示例。

---

## Key Anti-patterns to Avoid（来自 RESEARCH.md）

| Anti-pattern | 原因 | 正确做法 |
|-------------|------|----------|
| `page_size = 65536` 放在 `journal_mode = WAL` 之后 | WAL 启用后 page_size 被静默忽略（默认 4096）| `page_size` 必须第一个执行 |
| `locking_mode=EXCLUSIVE` 时验证代码与 exporter 共存 | 报 "database is locked" | `{}` 块确保 exporter drop 后再打开验证连接 |
| `batch_size` 默认 10_000 导致现有小样本测试不覆盖中间 COMMIT | 覆盖率盲区 | 新增专用测试用 `batch_size=2` 写 5 条 |
| 在 WAL 模式保留 `synchronous=OFF` | 崩溃时 WAL 可损坏 | 改为 `synchronous=NORMAL` |

---

## Metadata

**Analog search scope:** `src/exporter/`, `src/config.rs`, `benches/`
**Files scanned:** 4 primary files（sqlite.rs, config.rs, bench_sqlite.rs, BENCHMARKS.md）+ mod.rs 参考
**Pattern extraction date:** 2026-05-09

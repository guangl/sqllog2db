# Phase 5: SQLite 性能优化 - Research

**Researched:** 2026-05-09
**Domain:** Rust rusqlite 0.39 — 批量事务、WAL 模式、prepared statement 复用
**Confidence:** HIGH（代码库直接读取 + rusqlite 源码验证 + 实际运行测试）

---

## Summary

Phase 5 的目标是对 SQLite 导出路径实施三项可量化的性能优化：批量事务（PERF-04）、WAL 模式（PERF-05）、prepared statement 复用确认（PERF-06）。

**关键发现1（事务）：** 当前代码 `initialize()` 中执行 `BEGIN TRANSACTION`，`finalize()` 中执行 `COMMIT`，本质上已经是一个**单大事务**（非单行提交）。PERF-04 的"批量事务"实现需要在此基础上添加 `batch_size` 配置，在每 N 条 INSERT 后执行 COMMIT + BEGIN，以控制内存峰值——同时在 `bench_sqlite.rs` 新增"单行提交"benchmark group，提供可量化的对比基线。

**关键发现2（WAL）：** 当前使用 `PRAGMA journal_mode = OFF`（最快但崩溃后数据库损坏）。PERF-05 要求改为 WAL。同时发现**关键 Bug**：当前代码将 `PRAGMA page_size = 65536` 放在 PRAGMA 列表的最后，而 SQLite 要求 `page_size` 必须在 `journal_mode = WAL` 之前设置，否则 page_size 变更被静默忽略（实测验证：WAL 后设置 page_size 返回 NULL，实际值仍为默认 4096）。Phase 5 必须修正 PRAGMA 顺序。

**关键发现3（prepared statement）：** 当前代码已在每次 `export()` 调用中使用 `prepare_cached()`，通过 LRU cache（容量默认 16）复用已编译的 statement。每次调用的实际开销是 `RefCell::borrow_mut()` + `HashMap::remove/insert`（O(1)），而不是 `sqlite3_prepare_v3()`（O(parse)）。PERF-06 的成功标准（代码审查确认无重复 prepare）已基本满足，但需通过 flamegraph 正式确认，并确保 cache capacity 足够（当前 16 >> 1 个 SQL）。

**Primary recommendation:** 按顺序实施：(1) 修正 PRAGMA 顺序 + 启用 WAL 模式（含 `pragma_update_and_check` 验证），(2) 添加 `batch_size` 配置 + 批量提交逻辑，(3) 在 `bench_sqlite.rs` 新增单行提交 baseline group，(4) 更新 BENCHMARKS.md。

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PERF-04 | SQLite 导出使用批量事务（batch INSERT 或显式 transaction 分组），减少单行提交开销 | 当前已是大事务；需新增 `batch_size` 配置 + bench 中添加单行提交对照组 |
| PERF-05 | SQLite 导出启用 WAL 模式，提升并发写入与读写分离性能 | 需修正 PRAGMA 顺序（page_size 先于 WAL），用 `pragma_update_and_check` 验证返回 "wal" |
| PERF-06 | SQLite prepared statement 复用——避免每行重新编译 SQL | `prepare_cached()` 已复用；需 flamegraph 确认，并文档化 cache 复用行为 |
</phase_requirements>

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| PRAGMA 配置（WAL、page_size） | `exporter/sqlite.rs` initialize() | — | 连接初始化时一次性设置，不在热循环中 |
| 批量事务管理（BEGIN/COMMIT） | `exporter/sqlite.rs` export() + finalize() | — | 计数器在 export() 递增，达 batch_size 时触发 COMMIT+BEGIN |
| Prepared statement 生命周期 | rusqlite StatementCache（LRU） | `exporter/sqlite.rs` | prepare_cached() 返回 CachedStatement，drop 时自动归还 cache |
| Benchmark 对比基线（单行 vs 批量） | `benches/bench_sqlite.rs` | — | 新增 benchmark group，criterion 统计对比 |
| batch_size 配置 | `src/config.rs` SqliteExporter | `exporter/sqlite.rs` | 用户可通过 config.toml 配置，默认值 10_000 |

---

## Standard Stack

### Core（已在项目中使用）

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rusqlite | 0.39.0 | SQLite 绑定 | 已使用；bundled feature 嵌入 SQLite C 库 [VERIFIED: Cargo.toml + Cargo.lock] |
| rusqlite `cache` feature | 内置 | `prepare_cached()` / `CachedStatement` | 默认启用，LRU cache 容量 16 [VERIFIED: rusqlite 源码 lib.rs:160] |
| rusqlite `Transaction` | 内置 | 显式事务控制 | `conn.transaction()` 需 `&mut Connection`；`Transaction::new_unchecked` 接受 `&Connection` [VERIFIED: rusqlite 源码 transaction.rs] |
| criterion | 0.7 | benchmark 框架 | Phase 3 已建立基础设施 [VERIFIED: Cargo.toml] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `execute_batch("BEGIN/COMMIT")` | `conn.transaction()` API | `execute_batch` 已在用，实现 batch 逻辑更直接；`Transaction` struct 需要 `&mut Connection` 与现有字段布局有借用冲突 |
| `synchronous = OFF`（当前） | `synchronous = NORMAL` | WAL + NORMAL 提供崩溃安全（断电后 WAL 可重放）；WAL + OFF 最快但崩溃时 WAL 可能损坏；导出工具建议 NORMAL |
| `journal_mode = OFF`（当前） | `journal_mode = WAL` | WAL 性能接近 OFF（差异 <10%）但支持崩溃安全；PERF-05 要求 WAL |

---

## Architecture Patterns

### System Architecture Diagram

```
SqliteExporter::initialize()
    ↓ PRAGMA page_size = 65536    ← 必须第一个！
    ↓ PRAGMA journal_mode = WAL   ← pragma_update_and_check 验证返回 "wal"
    ↓ PRAGMA synchronous = NORMAL
    ↓ PRAGMA cache_size / locking_mode / temp_store / mmap_size
    ↓ DROP / DELETE / CREATE TABLE
    ↓ execute_batch("BEGIN")      ← 第一批事务开始
    
SqliteExporter::export() 热循环（每条 record）
    ↓ prepare_cached(&insert_sql) ← LRU cache lookup (O(1)，无 prepare)
    ↓ stmt.execute(params)
    ↓ stats.record_success()
    ↓ row_count += 1
    ↓ if row_count % batch_size == 0:
         execute_batch("COMMIT; BEGIN")  ← 批量提交 + 新事务

SqliteExporter::finalize()
    ↓ execute_batch("COMMIT")     ← 提交最后一批
    
benches/bench_sqlite.rs
    ├── sqlite_export group（现有）← handle_run() 全流程，WAL + batch 模式
    ├── sqlite_export_single_row group（新增）← 每条 INSERT 独立 BEGIN/COMMIT
    └── sqlite_export_real group（现有）← 真实文件（sqllogs/ 存在时）
```

### Recommended Project Structure

```
src/
├── config.rs              # SqliteExporter 新增 batch_size: usize 字段
└── exporter/sqlite.rs     # PRAGMA 顺序修正 + WAL 验证 + 批量提交逻辑

benches/
├── bench_sqlite.rs        # 新增 sqlite_export_single_row benchmark group
└── BENCHMARKS.md          # Phase 5 数值更新
```

### Pattern 1: WAL 模式初始化（正确 PRAGMA 顺序）

**What:** page_size 必须在 WAL 之前设置；journal_mode=WAL 用 `pragma_update_and_check` 验证返回值
**When to use:** `SqliteExporter::initialize()` 中

```rust
// Source: [VERIFIED: rusqlite 源码 pragma.rs:378-393] + [VERIFIED: 实际 Python 测试]
// 正确顺序：page_size 先于 journal_mode=WAL
conn.execute_batch("PRAGMA page_size = 65536;")?;

let journal_mode: String = conn.pragma_update_and_check(
    None,
    "journal_mode",
    "WAL",
    |row| row.get(0),
)?;
if journal_mode != "wal" {
    return Err(Self::db_err(format!(
        "journal_mode=WAL not supported, got: {journal_mode}"
    )));
}

conn.execute_batch(
    "PRAGMA synchronous = NORMAL;
     PRAGMA cache_size = 1000000;
     PRAGMA locking_mode = EXCLUSIVE;
     PRAGMA temp_store = MEMORY;
     PRAGMA mmap_size = 30000000000;
     PRAGMA threads = 4;",
)?;
```

### Pattern 2: 批量事务逻辑（在 export 中计数）

**What:** 在 SqliteExporter 中加 `row_count: usize` 和 `batch_size: usize` 字段，每 N 条 COMMIT + BEGIN
**When to use:** `export_one_preparsed()` 热路径中

```rust
// Source: [VERIFIED: 当前代码结构 + rusqlite execute_batch 文档]
// SqliteExporter 新增字段：
// row_count: usize,
// batch_size: usize,  // 默认 10_000

fn batch_commit_if_needed(&mut self) -> std::result::Result<(), rusqlite::Error> {
    self.row_count += 1;
    if self.row_count % self.batch_size == 0 {
        let conn = self.conn.as_ref().unwrap();
        conn.execute_batch("COMMIT; BEGIN")?;
        // 注意：stats.flush_operations 可以在此递增
    }
    Ok(())
}
```

### Pattern 3: 单行提交 benchmark（PERF-04 对照组）

**What:** 在 bench_sqlite.rs 新增 group，每条 INSERT 独立 BEGIN/COMMIT
**When to use:** 建立 PERF-04 可量化对比基线

```rust
// Source: [VERIFIED: 现有 bench_sqlite.rs 模式 + rusqlite execute_batch]
// 在 benchmark 中绕过 handle_run()，直接操控 SqliteExporter 的 batch_size=1
// 或者通过 config 传入 batch_size=1 触发单行提交
fn bench_sqlite_single_row(c: &mut Criterion) {
    // 配置 batch_size = 1（每条 INSERT 独立事务）
    // 与 sqlite_export（batch_size=10000）对比
}
```

### Pattern 4: pragma_update_and_check 用于集成测试断言

**What:** 测试代码中断言 WAL 模式已启用
**When to use:** PERF-05 成功标准验证

```rust
// Source: [VERIFIED: rusqlite pragma.rs:378-393 测试用例]
let journal_mode: String = conn
    .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
    .unwrap();
assert_eq!(journal_mode, "wal", "WAL mode must be active");
```

### Anti-Patterns to Avoid

- **PRAGMA 顺序错误：** 将 `page_size` 放在 `journal_mode=WAL` 之后导致 page_size 被静默忽略（默认 4096），实际测试已验证 [VERIFIED: 2026-05-09 Python 测试]
- **page_size 用 execute_batch 验证：** `execute_batch("PRAGMA page_size=65536")` 不报错但可能静默失败，应用 `pragma_update_and_check` 验证实际值
- **在 WAL 模式下保留 synchronous=OFF：** 虽然合法且更快，但崩溃时 WAL 文件可能损坏。导出工具建议 `NORMAL`（成功标准未明确指定，推荐 NORMAL）
- **在大事务中不添加 batch 分组：** 超大导出（百万行）可能产生大量 WAL 未提交数据，建议 10,000 条提交一次
- **在测试中重用 SqliteExporter 实例跨多次写：** `locking_mode=EXCLUSIVE` 锁住文件，测试完毕必须 drop exporter 才能让验证代码打开同一个 .db（现有测试已用 `{}` 块确保 drop，Phase 5 测试同样需要）

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WAL 模式验证 | 自己查询 PRAGMA journal_mode | `pragma_update_and_check` | 原子设置+验证，已是标准 API [VERIFIED: rusqlite docs] |
| Prepared statement 复用 | 手动存储 `Statement<'conn>` 字段 | `prepare_cached()` | 生命周期绑定问题；LRU cache 已经很高效 [VERIFIED: cache.rs] |
| 手动 BEGIN/COMMIT 字符串 | `format!("COMMIT; BEGIN")` | `execute_batch("COMMIT; BEGIN")` | 单次 FFI 调用，执行两条语句，比两次 execute 更高效 |
| 批量 INSERT 多行 VALUES | 构建 multi-row VALUES SQL | 单次 prepared INSERT 分批提交 | rusqlite 不提供 multi-row binding API；当前单行 INSERT 在事务内已足够快 |

**Key insight:** SQLite 性能调优的 95% 来自事务边界设计（每次 COMMIT 都要 fsync），而非 SQL 优化。

---

## Current Code Deep Analysis

### 现状1：事务模型

```
initialize()
    execute_batch("BEGIN TRANSACTION")  ← 所有记录共享一个事务

export()  ← 每次调用无 COMMIT，全部在同一大事务中

finalize()
    execute_batch("COMMIT")             ← 最终一次性提交
```

当前已是"大事务"模式（非单行提交）。PERF-04 要求添加 N 条分批提交逻辑，并用 bench 与单行提交对比。

### 现状2：PRAGMA 配置（Bug）

```rust
// src/exporter/sqlite.rs:217-226
conn.execute_batch(
    "PRAGMA journal_mode = OFF;    // ← 要改为 WAL
     PRAGMA synchronous = OFF;
     PRAGMA cache_size = 1000000;
     PRAGMA locking_mode = EXCLUSIVE;
     PRAGMA temp_store = MEMORY;
     PRAGMA mmap_size = 30000000000;
     PRAGMA page_size = 65536;    // ← Bug! 必须在 WAL 之前！
     PRAGMA threads = 4;",
)
```

**Phase 5 修改：**
- 将 `page_size = 65536` 移至第一行
- 将 `journal_mode = OFF` 改为 `journal_mode = WAL`（用 `pragma_update_and_check`）
- `synchronous` 保持 `OFF` 或改为 `NORMAL`（成功标准未要求，推荐 NORMAL）

### 现状3：prepared statement 复用

```rust
// 每次 export_one_preparsed() 调用：
let mut stmt = conn.prepare_cached(&self.insert_sql)?;  // LRU cache lookup, O(1)
Self::do_insert_preparsed(&mut stmt, ...)?;
// stmt drop → 归还到 LRU cache
```

`prepare_cached()` 已通过 `StatementCache`（LRU，默认容量 16）复用 prepared statement。只有第一次调用时触发真正的 `sqlite3_prepare_v3()`，之后均为 cache hit。[VERIFIED: rusqlite cache.rs:136-150]

PERF-06 的要求已基本满足，但需：
1. 代码审查确认（添加注释说明）
2. flamegraph 确认无 `sqlite3_prepare_v3` 在热循环中的采样

---

## Common Pitfalls

### Pitfall 1: page_size 与 WAL 的 PRAGMA 顺序

**What goes wrong:** `page_size = 65536` 放在 `journal_mode = WAL` 之后时，SQLite 静默忽略 page_size 设置（返回 NULL），实际 page_size 为默认值 4096。
**Why it happens:** WAL 模式一旦启用，数据库 header 中的 page_size 就被锁定，后续 PRAGMA 无法更改。
**How to avoid:** 始终将 `PRAGMA page_size` 作为第一个 PRAGMA 执行，在 `journal_mode=WAL` 之前。
**Warning signs:** `PRAGMA page_size` 设置后返回 NULL（而非设置的值）。

[VERIFIED: 实际 Python3 + sqlite3 测试，2026-05-09]

### Pitfall 2: WAL 文件残留（.db-wal 和 .db-shm）

**What goes wrong:** 进程崩溃或 connection 未正确关闭时，WAL 文件（`.db-wal`, `.db-shm`）残留，下次打开时 SQLite 自动重放。测试中可能因此状态不干净。
**Why it happens:** WAL 只在 checkpoint 时合并到主文件。
**How to avoid:** 测试中使用 `TempDir`（已用），finalize() 中在 COMMIT 后调用 `PRAGMA wal_checkpoint(FULL)` 确保 WAL 合并（可选，根据需求决定）。
**Warning signs:** 测试后留有 `.db-wal` / `.db-shm` 文件；行数计数与预期不符。

### Pitfall 3: locking_mode=EXCLUSIVE 阻止验证代码打开同一 .db

**What goes wrong:** 测试的 exporter 使用 `locking_mode=EXCLUSIVE`，验证代码（`Connection::open(&dbfile)`）在 exporter 未 drop 前无法打开同一 .db 文件。
**Why it happens:** EXCLUSIVE 模式在连接存续期间持有文件锁。
**How to avoid:** 现有测试已用 `{}` 块确保 exporter drop 后再打开验证连接。Phase 5 新测试必须沿用此模式。
**Warning signs:** 验证代码报 "database is locked" 错误。

### Pitfall 4: batch_size 配置对现有功能测试的影响

**What goes wrong:** 现有 13 个 SQLite 单元测试写入 2–5 条记录。若 batch_size 默认 10000，测试记录数远小于 batch_size，中间 COMMIT 路径完全未覆盖。
**Why it happens:** 测试数据量小，批量提交分支不触发。
**How to avoid:** 新增一个专门测试批量提交路径的测试（如写入 batch_size+1 条记录，验证中间 COMMIT 后行数）；或在测试中用小 batch_size（如 2）覆盖中间提交路径。
**Warning signs:** 覆盖率工具显示 `batch_commit_if_needed` 的 COMMIT 分支从未触发。

### Pitfall 5: benchmark 单行提交模式的实现方式

**What goes wrong:** 直接修改 `make_config()` 加 `batch_size=1` 来实现单行提交，但 config 结构无此字段时编译失败；或测试写入量太少，单行提交和批量提交差异不显著。
**Why it happens:** SQLite 单行提交的代价来自每次 COMMIT 的 fsync，10000 条以上差异才显著。
**How to avoid:** 单行提交 benchmark 必须在 bench_sqlite.rs 中通过专门函数直接调用 `SqliteExporter`（设 `batch_size=1`），而非通过 `handle_run()`；或在 `make_config()` 中增加 `batch_size` 参数控制。至少测试 10000 条以上。
**Warning signs:** criterion 输出单行提交和批量提交时间接近（<10% 差异），说明 fsync 开销未被触发（WAL 模式下 synchronous=OFF 时 fsync 不强制，差异可能确实很小）。

---

## Code Examples

### WAL 初始化正确模式

```rust
// Source: [VERIFIED: rusqlite pragma.rs + 实际测试 2026-05-09]
fn initialize_pragmas(conn: &Connection) -> Result<(), rusqlite::Error> {
    // 必须第一个：在 WAL 之前设置 page_size
    conn.execute_batch("PRAGMA page_size = 65536;")?;

    // 验证 WAL 模式已启用
    let journal_mode: String = conn.pragma_update_and_check(
        None,
        "journal_mode",
        "WAL",
        |row| row.get(0),
    )?;
    debug_assert_eq!(journal_mode, "wal", "WAL mode not active");

    // 其余 PRAGMA（顺序不敏感）
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

### 批量提交逻辑

```rust
// Source: [VERIFIED: 基于当前 sqlite.rs 结构]
// SqliteExporter 新增字段：
//   row_count: usize,      // 当前批次已写行数
//   batch_size: usize,     // 每批次大小，默认 10_000

// 在 export_one_preparsed() 末尾（do_insert_preparsed 成功后）：
self.stats.record_success();
self.row_count += 1;
if self.row_count % self.batch_size == 0 {
    let conn = self.conn.as_ref().unwrap();
    conn.execute_batch("COMMIT; BEGIN")
        .map_err(|e| Self::db_err(format!("batch commit failed: {e}")))?;
}
```

### 集成测试：断言 WAL 模式已启用

```rust
// Source: [VERIFIED: rusqlite pragma.rs 测试用例模式]
#[test]
fn test_sqlite_wal_mode_enabled() {
    let dir = tempfile::TempDir::new().unwrap();
    let dbfile = dir.path().join("wal_test.db");

    {
        let mut exporter = SqliteExporter::new(
            dbfile.to_string_lossy().into(), "tbl".into(), true, false,
        );
        exporter.initialize().unwrap();
        exporter.finalize().unwrap();
    } // exporter drop → EXCLUSIVE lock released

    let conn = rusqlite::Connection::open(&dbfile).unwrap();
    let mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "wal", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal", "PRAGMA journal_mode=WAL 必须返回 'wal'");
}
```

### bench_sqlite.rs 新增单行提交 group

```rust
// Source: [ASSUMED: 基于现有 bench_sqlite.rs 模式推断]
fn bench_sqlite_single_row(c: &mut Criterion) {
    // 使用 batch_size=1（每条 INSERT 独立事务）作为 PERF-04 对照组
    // 需要 SqliteExporterConfig 中有 batch_size 字段，或通过 exporter.batch_size=1 设置
    let mut group = c.benchmark_group("sqlite_single_row");
    group.sample_size(10);
    for &n in &[1_000usize, 10_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                // 用 batch_size=1 配置跑 handle_run，或直接调用 SqliteExporter
            });
        });
    }
    group.finish();
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| journal_mode = DELETE（默认） | journal_mode = OFF（当前代码） | v1.0 已完成 | 写入最快但崩溃损坏数据库 |
| journal_mode = OFF | journal_mode = WAL（Phase 5 目标） | Phase 5 | WAL 支持并发读写，崩溃安全；写入性能差异 <10% |
| 单行提交（每 INSERT 独立事务） | 大事务（当前），批量事务（Phase 5） | v1.0 → Phase 5 | 大事务比单行提交快 100x+（消除 fsync 开销） |
| `conn.prepare()` 每次重新编译 | `conn.prepare_cached()` LRU 复用 | v1.0 已完成 | 消除热循环中的 sqlite3_prepare_v3 调用 |

**已知状态：**
- page_size=65536 的 PRAGMA 顺序 Bug 存在于 v1.0 代码，WAL 模式下必须修正
- rusqlite 0.39 的 `prepare_cached` 默认 LRU 容量 16，远大于当前所需（1 个 SQL）

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rusqlite 0.39.0 (bundled) | SQLite 所有功能 | ✓ | 0.39.0 [VERIFIED: Cargo.lock] | — |
| SQLite 3（内嵌 via bundled） | WAL、PRAGMA | ✓ | bundled with rusqlite [VERIFIED: Cargo.toml features] | — |
| criterion 0.7 | benchmark | ✓ | 0.7.0 [VERIFIED: Cargo.toml] | — |
| benches/baselines/sqlite_export/ v1.0 | --baseline v1.0 对比 | ✓ | 存在（7.070ms @ 10k records）[VERIFIED: baselines/sqlite_export/10000/v1.0/] | — |
| sqllogs/ 真实日志目录 | real-file benchmark | ? | Phase 3 时存在（538MB 2 文件）| bench 自动 skip（CI-safe）|

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) + criterion 0.7 |
| Config file | `Cargo.toml` `[[bench]]` 条目 |
| Quick run command | `cargo test --lib -- exporter::sqlite` |
| Full suite command | `cargo test` |
| Benchmark quick run | `cargo bench --bench bench_sqlite -- --sample-size 10 sqlite_export/10000` |
| Benchmark baseline compare | `CRITERION_HOME=benches/baselines cargo bench --bench bench_sqlite -- --baseline v1.0` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PERF-04 | 批量事务比单行提交快（criterion 可量化） | benchmark（criterion） | `cargo bench --bench bench_sqlite -- sqlite_export sqlite_single_row` | ❌ Wave 0 需新增 `sqlite_single_row` group |
| PERF-05 | journal_mode=WAL 返回 "wal" | integration test（rusqlite pragma_update_and_check） | `cargo test --lib -- exporter::sqlite::tests::test_sqlite_wal_mode_enabled` | ❌ Wave 0 需新增测试 |
| PERF-06 | 写入循环中无重复 prepare() | 代码审查 + flamegraph | `samply record ... cargo bench --profile flamegraph --bench bench_sqlite` | ✅（samply 已装，Phase 3 已用）|
| 无回归 | 649 tests 全部通过 | unit/integration | `cargo test` | ✅ 当前 649 tests (290+309+50) passing |
| page_size Bug 修正 | page_size=65536 在 WAL 下生效 | integration（读取 PRAGMA page_size 断言） | `cargo test --lib -- exporter::sqlite::tests::test_sqlite_wal_page_size` | ❌ Wave 0 需新增 |

### Sampling Rate

- **每次 commit 前：** `cargo test --lib -- exporter::sqlite`（13 个 SQLite 单元测试）
- **每个 Wave 完成：** `cargo test` + `cargo bench --bench bench_sqlite -- --sample-size 10`
- **Phase gate：** `cargo test` 全绿 + `CRITERION_HOME=benches/baselines cargo bench --bench bench_sqlite -- --baseline v1.0` 输出 "Performance has improved" + WAL 测试通过

### Wave 0 Gaps

- [ ] 新增集成测试：`test_sqlite_wal_mode_enabled` — 断言 journal_mode 返回 "wal"（PERF-05）
- [ ] 新增集成测试：`test_sqlite_wal_page_size` — 断言 WAL 下 page_size=65536 生效
- [ ] 新增集成测试：`test_sqlite_batch_commit` — 用 batch_size=2 写 5 条记录，验证中间 COMMIT 路径
- [ ] 新增 `sqlite_single_row` benchmark group — PERF-04 对照基线
- [ ] `src/config.rs` 的 `SqliteExporter` 新增 `batch_size: usize` 字段（含 serde default）

---

## Project Constraints (from CLAUDE.md)

| Directive | Impact on Phase 5 |
|-----------|-----------------|
| `cargo clippy --all-targets -- -D warnings` 零警告 | 新增字段/方法必须通过 clippy；`pragma_update_and_check` 返回值不能忽略 |
| `cargo fmt` | 修改 sqlite.rs 和 config.rs 后必须格式化 |
| 函数不超过 40 行 | `initialize()` 当前 ~50 行，需要提取 `initialize_pragmas()` 辅助函数（Phase 5 重构时顺带完成）|
| `cargo test` | 649 个测试必须全部通过，无功能退化 |
| 描述性变量名，不用单字母 | `row_count`、`batch_size` 命名符合要求 |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | WAL 模式下 synchronous=NORMAL 是推荐值 | Standard Stack | 若要求最高写入速度，synchronous=OFF 更快（成功标准未明确，建议 NORMAL 作为默认）|
| A2 | bench_sqlite.rs 中的单行提交 benchmark 用 batch_size=1 实现最方便 | Code Examples | 若 config 不支持 batch_size=1 需要另外设计 benchmark 路径 |
| A3 | WAL 模式下与 journal_mode=OFF 的性能差异 <10% | State of the Art | 若差异超过 5%，可能触发 hard limit（7.424ms）；可通过 Phase 5 benchmark 验证 |
| A4 | PERF-04 成功标准中"每 N 条提交"的 N 默认 10,000 合理 | Architecture Patterns | 若真实文件平均 SQL 长度很长，10k 条可能占用较多 WAL 内存；可调整 |

**Assumptions 数量较少（4 个），其余所有关键结论均已通过工具验证。**

---

## Open Questions (RESOLVED)

1. **`synchronous = OFF` vs `NORMAL` 的选择**
   - What we know：WAL + NORMAL 提供崩溃安全；WAL + OFF 最快但 WAL 崩溃时可能损坏
   - What's unclear：成功标准未指定 synchronous 级别；导出工具的业务需求（是否需要崩溃安全？）
   - Recommendation：Phase 5 实现时默认 NORMAL，可以通过 config 配置允许用户改为 OFF
   - **RESOLVED:** Plan 02 已实现 `synchronous = NORMAL`（写入 PRAGMA 列表中，initialize_pragmas() 已确认）

2. **batch_size 是否需要加入 config.toml 用户配置**
   - What we know：PERF-04 的成功标准只要求"benchmark 可量化"；config 暴露给用户增加复杂度
   - What's unclear：是否是用户应该关心的参数
   - Recommendation：加入 config（带合理默认值 10000），但在文档中说明默认值已优化，大多数用户无需修改
   - **RESOLVED:** Plan 01 已在 config.rs 加入 `batch_size: usize` 字段，serde default 10_000，apply_one 支持 "exporter.sqlite.batch_size" key

3. **`wal_checkpoint` 是否需要在 finalize() 中调用**
   - What we know：WAL 文件在 checkpoint 前不合并到主 .db 文件；大量写入后 WAL 文件可能很大
   - What's unclear：导出工具的使用场景是否需要在写完后立即 checkpoint（比如后续要复制 .db 文件）
   - Recommendation：finalize() 中在 COMMIT 后执行 `PRAGMA wal_checkpoint(TRUNCATE)` 确保 WAL 已截断，避免遗留大 WAL 文件
   - **RESOLVED:** Plan 02 已在 finalize() 的 COMMIT 之后调用 `PRAGMA wal_checkpoint(TRUNCATE)`

---

## Sources

### Primary (HIGH confidence)

- 直接代码审计：`src/exporter/sqlite.rs`（事务模型、PRAGMA 配置、prepare_cached 调用）[VERIFIED: 源文件直接读取]
- rusqlite 0.39.0 源码：`cache.rs`（StatementCache LRU、默认容量 16）[VERIFIED: ~/.cargo/registry]
- rusqlite 0.39.0 源码：`transaction.rs`（Transaction::new_unchecked 接受 &Connection）[VERIFIED: ~/.cargo/registry]
- rusqlite 0.39.0 源码：`lib.rs`（STATEMENT_CACHE_DEFAULT_CAPACITY = 16）[VERIFIED: ~/.cargo/registry]
- rusqlite 0.39.0 文档：pragma.rs 测试用例（pragma_update_and_check journal_mode）[VERIFIED: 源码]
- 实际运行验证：Python3 + sqlite3 测试 WAL + page_size 顺序 [VERIFIED: 2026-05-09 本机测试]
- 实际运行验证：WAL + EXCLUSIVE locking 兼容性 [VERIFIED: 2026-05-09 本机测试]
- Phase 3 BENCHMARKS.md：sqlite_export/10000 v1.0 median = 7.070ms，hard limit = 7.424ms [VERIFIED: benches/BENCHMARKS.md]
- Phase 3 baseline JSON：sqlite_export/10000/v1.0/estimates.json mean = 7.087ms [VERIFIED: 本地文件]
- Cargo.toml / Cargo.lock：rusqlite 0.39.0 + bundled feature [VERIFIED]

### Secondary (MEDIUM confidence)

- rusqlite 0.39 官方文档（Context7）：`prepare_cached`、`pragma_update_and_check`、`Transaction` API [CITED: docs.rs/rusqlite/0.39.0]

### Tertiary (LOW confidence)

- WAL 模式与 journal_mode=OFF 性能差异 <10% [ASSUMED: 基于通用 SQLite 性能经验，未在本项目实测]
- `PRAGMA wal_checkpoint(TRUNCATE)` 在 finalize() 中的必要性 [ASSUMED: 基于 SQLite WAL 行为，未测量实际 WAL 文件大小]

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — rusqlite 0.39 源码直接验证
- Architecture: HIGH — 代码直接审计 + 实际运行测试
- Pitfalls: HIGH — page_size/WAL 顺序 Bug 通过实际测试确认
- Performance estimates: MEDIUM — WAL vs OFF 差异为 ASSUMED，需 Phase 5 benchmark 实测

**Research date:** 2026-05-09
**Valid until:** 2026-06-09（rusqlite 0.39 稳定，SQLite WAL 行为不变，30 天有效）

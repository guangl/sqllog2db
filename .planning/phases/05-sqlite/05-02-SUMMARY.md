---
phase: 05-sqlite
plan: "02"
subsystem: exporter/sqlite
tags: [sqlite, wal, pragma, batch-commit, performance]
dependency_graph:
  requires: [05-01]
  provides: [initialize_pragmas, batch_commit_if_needed, wal_checkpoint]
  affects: [src/exporter/sqlite.rs]
tech_stack:
  added: []
  patterns:
    - "initialize_pragmas() 模块级私有函数，page_size 在 WAL 之前，pragma_update_and_check 验证"
    - "作用域块释放 stmt/conn 借用，再调用 &mut self 方法（Rust 借用规则）"
    - "finalize() 在 COMMIT 后执行 wal_checkpoint(TRUNCATE) 截断 WAL 文件"
key_files:
  created: []
  modified:
    - src/exporter/sqlite.rs
decisions:
  - "用作用域块 {} 包裹 stmt/conn，确保 CachedStatement drop 后才调用 batch_commit_if_needed（解决借用冲突）"
  - "initialize_pragmas() 使用 pragma_update_and_check 而非 execute_batch，获得原子设置+验证语义"
  - "batch_commit_if_needed 在 record_success() 之后调用，保持统计和提交行为的一致性"
metrics:
  duration: "5m 23s"
  completed: "2026-05-10"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 1
---

# Phase 05 Plan 02: SQLite 性能优化（WAL + 批量事务 + WAL Checkpoint）Summary

WAL 模式替换 journal_mode=OFF + page_size 顺序修正 + 批量事务 + finalize WAL checkpoint，新增 3 个集成测试覆盖 PERF-04/05/06。

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | 结构体字段 + new/from_config + initialize_pragmas() | 29a1a0d | src/exporter/sqlite.rs |
| 2 | batch_commit_if_needed() + finalize WAL checkpoint + 3 集成测试 | 631db3b | src/exporter/sqlite.rs |

## What Was Built

### Task 1: initialize_pragmas() + 结构体字段

- `SqliteExporter` 新增 `row_count: usize` 和 `batch_size: usize` 字段
- `new()` 初始化 `row_count: 0, batch_size: 10_000`（函数签名不变）
- `from_config()` 从 config 读取 `batch_size`
- 提取模块级私有函数 `initialize_pragmas(&Connection)`：
  - `PRAGMA page_size = 65536` 先于 WAL（修正原 Bug：page_size 必须在 WAL 启用前设置）
  - `pragma_update_and_check(None, "journal_mode", "WAL", ...)` 原子设置并验证返回值 == "wal"
  - WAL 验证失败返回 `Err(rusqlite::Error::SqliteFailure(...))`
  - 其余 PRAGMA：synchronous=NORMAL, cache_size, locking_mode=EXCLUSIVE, temp_store, mmap_size, threads
- `initialize()` 替换原 `execute_batch("PRAGMA journal_mode=OFF; ...")` 为 `initialize_pragmas(&conn)?`
- `initialize()` 设置 `self.conn = Some(conn)` 后重置 `self.row_count = 0`

### Task 2: batch_commit_if_needed() + finalize WAL checkpoint

- `batch_commit_if_needed(&mut self) -> Result<()>`：递增 `row_count`，若整除 `batch_size` 则执行 `COMMIT; BEGIN`
- 三个 export 路径均调用（作用域块解决借用冲突）：
  - `export()`: stmt 在 `{}` 内 drop → `self.batch_commit_if_needed()?`
  - `export_one_normalized()`: 同上
  - `export_one_preparsed()`: 同上
- `finalize()` 在 `COMMIT` 后追加 `PRAGMA wal_checkpoint(TRUNCATE);`（避免遗留 .db-wal 文件）
- 新增 3 个集成测试：
  - `test_sqlite_wal_mode_enabled`：断言 `journal_mode == "wal"`
  - `test_sqlite_wal_page_size`：断言 `page_size == 65536`
  - `test_sqlite_batch_commit`：batch_size=2 写 5 条，断言 COUNT=5

## Verification Results

```
cargo test --lib -- exporter::sqlite
test result: ok. 16 passed; 0 failed (比 Plan 01 完成时多 3 个)

cargo test
test result: ok. 293+312+50 = 655 passed; 0 failed (无回归)

cargo clippy --all-targets -- -D warnings
Finished dev profile — 零警告
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Rust 借用冲突：CachedStatement 持有 conn 的不可变借用，导致 batch_commit_if_needed() 无法获取 &mut self**

- **Found during:** Task 2 编译阶段
- **Issue:** `stmt` (CachedStatement) 在 export 方法中通过 `self.conn.as_ref()` 持有不可变借用，而 `batch_commit_if_needed()` 需要 `&mut self`，Rust 借用检查器拒绝编译
- **Fix:** 用显式作用域块 `{}` 包裹 `conn` 和 `stmt`，确保 `stmt` drop 后才调用 `self.batch_commit_if_needed()`；这是正确的 Rust 惯用做法，不改变任何行为
- **Files modified:** src/exporter/sqlite.rs（export/export_one_normalized/export_one_preparsed 三个方法）
- **Commit:** 631db3b（同 Task 2 主提交）

**2. [Rule 1 - Bug] clippy::doc_markdown 警告：注释中技术词汇未加反引号**

- **Found during:** Task 1 clippy 阶段
- **Issue:** 初始函数注释中 `SQLite`、`page_size`（无反引号）触发 `-D warnings` 编译失败
- **Fix:** 将注释改写为 "按正确顺序设置数据库 PRAGMA：`page_size` 必须在 `journal_mode=WAL` 之前"
- **Files modified:** src/exporter/sqlite.rs（initialize_pragmas 函数文档注释）
- **Commit:** 29a1a0d（同 Task 1 主提交）

## Threat Model Coverage

| Threat ID | Status | Evidence |
|-----------|--------|---------|
| T-05-04 | mitigated | initialize_pragmas() 中 page_size 在 WAL 之前（行 28 < 行 32） |
| T-05-05 | mitigated | pragma_update_and_check 返回值 != "wal" 时 return Err(...) |
| T-05-06 | mitigated | finalize() COMMIT 后执行 wal_checkpoint(TRUNCATE) |
| T-05-07 | mitigated | config.rs validate() 检查 batch_size > 0（Plan 01 已实现，行 404） |
| T-05-08 | accepted | 设计选择，文档化；{} 块确保 EXCLUSIVE 锁随 drop 释放 |

## Self-Check: PASSED

- [x] src/exporter/sqlite.rs 存在且包含所有新代码
- [x] 提交 29a1a0d 存在（Task 1）
- [x] 提交 631db3b 存在（Task 2）
- [x] `grep -c "initialize_pragmas"` → 2（定义+调用）
- [x] `grep -c "batch_commit_if_needed"` → 4（定义+3处调用）
- [x] `grep -c "wal_checkpoint"` → 1（finalize 中）
- [x] 新增 3 个测试，total 16 通过
- [x] 全套 655 测试通过，clippy 零警告

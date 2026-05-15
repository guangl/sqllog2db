---
phase: 07-tech-debt-fix
plan: "01"
subsystem: exporter/sqlite
tags:
  - sqlite
  - sql-injection
  - error-handling
  - validation
  - tech-debt
dependency_graph:
  requires: []
  provides:
    - SqliteExporter::validate() ASCII 标识符校验
    - handle_delete_clear_result() helper（DEBT-01 软失败语义）
    - prepare_target_table() helper（行数约束满足）
    - DDL 全量双引号转义（5 处 format!）
  affects:
    - src/config.rs
    - src/exporter/sqlite.rs
tech_stack:
  added: []
  patterns:
    - is_some_and + is_ascii_alphabetic/is_ascii_alphanumeric（不引入 regex crate 的标识符校验）
    - rusqlite::Error::SqliteFailure 模式匹配（区分 no such table 与真实错误）
    - SQLite 双引号标识符语法（`"table_name"` 转义）
key_files:
  created: []
  modified:
    - src/config.rs
    - src/exporter/sqlite.rs
decisions:
  - "使用 is_some_and（Rust 1.70+）替代 map_or(false, ...)，更简洁且 clippy 推荐"
  - "将 prepare_target_table() 提取为 helper 以保证 initialize() ≤ 40 行（CLAUDE.md 约束）"
  - "handle_delete_clear_result 采用软失败语义（D-01 明确决策）：DELETE 失败不阻断导出流程"
  - "test_sqlite_initialize_creates_quoted_table 断言改为检查 sqlite_master.sql 中含 \"my_records\"，因 SQLite 在存储时去掉 IF NOT EXISTS 子句"
  - "test_sqlite_initialize_silent_when_table_missing 使用 {} 代码块确保 exporter 在验证查询前 drop（释放 EXCLUSIVE lock）"
metrics:
  duration_seconds: 473
  completed_date: "2026-05-10"
  tasks_completed: 3
  tasks_total: 3
  files_changed: 2
  tests_added: 11
  tests_total: 673
---

# Phase 7 Plan 01: SQLite Tech-Debt Fix Summary

SQLite 导出器双重技术债修复：ASCII 标识符白名单校验消除 SQL 注入向量，显式 match 替换静默 `let _ =` 模式实现错误可观测性，全部 5 处 DDL/DML 字符串应用双引号转义纵深防御。

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | SqliteExporter::validate() ASCII 标识符校验 | fb5532f | src/config.rs |
| 2 | DDL/DML format! 双引号转义（4 处 + 3 项测试更新 + 1 项新测试） | 0aeba09 | src/exporter/sqlite.rs |
| 3 | DELETE FROM 静默错误改为显式 match + helper 提取 | 0d27afb | src/exporter/sqlite.rs |

## Implementation Details

### Task 1 — config.rs 变更（DEBT-02 D-04）

**位置：** `src/config.rs` Lines 397–414（`SqliteExporter::validate()`）

插入了 `is_valid_ident` 块（在 `table_name.trim().is_empty()` 检查之后、`batch_size == 0` 检查之前）：

```rust
let is_valid_ident = {
    let mut chars = self.table_name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
};
if !is_valid_ident {
    return Err(Error::Config(ConfigError::InvalidValue {
        field: "exporter.sqlite.table_name".to_string(),
        value: self.table_name.clone(),
        reason: "table name must match ^[a-zA-Z_][a-zA-Z0-9_]*$ (ASCII identifiers only)"
            .to_string(),
    }));
}
```

新增 8 项测试：
- `test_validate_sqlite_table_name_valid_simple` — "tbl" → Ok
- `test_validate_sqlite_table_name_valid_underscore_prefix` — "_records" → Ok
- `test_validate_sqlite_table_name_valid_with_digits` — "t1_log_2024" → Ok
- `test_validate_sqlite_table_name_rejects_leading_digit` — "1tbl" → Err
- `test_validate_sqlite_table_name_rejects_special_char` — "tbl;DROP" → Err
- `test_validate_sqlite_table_name_rejects_quote` — "tbl\"x" → Err
- `test_validate_sqlite_table_name_rejects_non_ascii` — "日志表" → Err
- `test_validate_sqlite_table_name_rejects_space` — "my tbl" → Err

### Task 2 — sqlite.rs DDL 双引号转义（DEBT-02 D-05）

5 处 `format!` 全部更改（其中 4 处在本 Task，第 5 处 DELETE FROM 在 Task 3 处理）：

| 位置 | 修改前 | 修改后 |
|------|--------|--------|
| `new()` insert_sql | `INSERT INTO {table_name} VALUES...` | `INSERT INTO "{table_name}" VALUES...` |
| `build_insert_sql()` 全量路径 | `INSERT INTO {table_name} VALUES...` | `INSERT INTO "{table_name}" VALUES...` |
| `build_insert_sql()` 投影路径 | `INSERT INTO {table_name} ({}) VALUES...` | `INSERT INTO "{table_name}" ({}) VALUES...` |
| `build_create_sql()` | `CREATE TABLE IF NOT EXISTS {table_name} ({})` | `CREATE TABLE IF NOT EXISTS "{table_name}" ({})` |
| `initialize()` DROP TABLE | `DROP TABLE IF EXISTS {}` | `DROP TABLE IF EXISTS "{}"` |

新增 `test_sqlite_initialize_creates_quoted_table`：验证 `sqlite_master.sql` 中包含 `"my_records"`（带双引号）。

### Task 3 — DELETE FROM 显式 match + helper（DEBT-01 D-01/D-02）

**新增 `handle_delete_clear_result()` helper（约 Line 218）：**

```rust
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
```

**新增 `prepare_target_table()` helper（约 Line 234）：** 将 `if self.overwrite / else if !self.append` 条件块提取出来，保证 `initialize()` 函数体 33 行（≤ 40 行 CLAUDE.md 约束）。

**DELETE FROM 修改（DEBT-02 最后一处转义）：**
```rust
Self::handle_delete_clear_result(
    self.conn.as_ref().unwrap().execute(
        &format!("DELETE FROM \"{}\"", self.table_name),
        [],
    ),
    &self.table_name,
);
```

新增 2 项测试：
- `test_sqlite_initialize_silent_when_table_missing`：全新 DB，overwrite=false、append=false，`initialize()` 返回 Ok（"no such table" 被静默）
- `test_sqlite_initialize_clears_existing_table_via_delete`：两次 run，期望最终行数 4（DELETE 清空了旧数据）

## Verification Results

### 全项目测试（673 个测试全绿）

```
test result: ok. 302 passed; 0 failed; 0 ignored; 0 measured
test result: ok. 321 passed; 0 failed; 0 ignored; 0 measured
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured
```

（原 651 个测试 + 本次新增 11 个 = 662 个 lib 测试 + 11 个 integration 测试，
实际 302+321 = 623 lib 测试 + 50 integration = 673 总计）

### cargo clippy --all-targets -- -D warnings

```
0 warnings, 0 errors
```

### cargo fmt -- --check

```
0 行差异
```

### ROADMAP Phase 7 成功标准验证

**标准 1 — 首次运行无害错误静默继续（TRUE）**

```
test exporter::sqlite::tests::test_sqlite_initialize_silent_when_table_missing ... ok
```

全新 DB 文件，DELETE FROM 触发 "no such table"，被静默，`initialize()` 返回 Ok。

**标准 2 — 真实错误写入 error log（TRUE）**

```bash
$ grep -c 'log::warn!("sqlite clear failed' src/exporter/sqlite.rs
1
```

`handle_delete_clear_result()` 中的 `if let Err(e) = result { log::warn!("sqlite clear failed for table {table_name}: {e}"); }` 确保非 "no such table" 的真实错误通过 `log::warn!` 写入应用日志（`[logging] file` 配置路径）。

**标准 3 — 非法 table_name 启动报错（TRUE）**

```
$ cargo run -- validate -c /tmp/sqllog2db-p7-test/illegal.toml
Error: Configuration error: Invalid configuration value exporter.sqlite.table_name = 'tbl;DROP TABLE x': table name must match ^[a-zA-Z_][a-zA-Z0-9_]*$ (ASCII identifiers only)
Exit: 2
```

合法配置：Exit 0；非法配置：Exit 2。

**标准 4 — DDL 全量双引号转义（TRUE）**

```bash
$ grep -n '"DELETE FROM\|INSERT INTO\|CREATE TABLE\|DROP TABLE' src/exporter/sqlite.rs
52:  "INSERT INTO \"{table_name}\" VALUES..."
76:  "INSERT INTO \"{table_name}\" VALUES..."
82:  "INSERT INTO \"{table_name}\" ({}) VALUES..."
112: "CREATE TABLE IF NOT EXISTS \"{table_name}\" ({})"
238: "DROP TABLE IF EXISTS \"{}\""
245: "DELETE FROM \"{}\""
```

5 处 format!（new + build_insert_sql×2 + build_create_sql + DROP + DELETE）全部使用双引号转义。

### initialize() 函数体行数

```bash
$ awk '/    fn initialize/,/^    fn export/' src/exporter/sqlite.rs | wc -l
35
```

35 行 ≤ 40 行（满足 CLAUDE.md 函数长度约束）。

### let _ = conn.execute 静默模式清零

```bash
$ grep -c 'let _ = conn.execute' src/exporter/sqlite.rs
0
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] test_sqlite_initialize_creates_quoted_table 断言修正**
- **Found during:** Task 2
- **Issue:** 计划中测试断言 `create_stmt.contains("CREATE TABLE IF NOT EXISTS \"my_records\"")` 失败，因为 SQLite 在 `sqlite_master` 中存储 CREATE 语句时省略了 `IF NOT EXISTS` 子句，实际存储为 `CREATE TABLE "my_records" (...)`
- **Fix:** 改为 `create_stmt.contains("\"my_records\"")` 验证双引号存在即可，不依赖具体 SQL 格式
- **Files modified:** src/exporter/sqlite.rs
- **Commit:** 0aeba09

**2. [Rule 1 - Bug] test_sqlite_initialize_silent_when_table_missing 数据库锁定修正**
- **Found during:** Task 3
- **Issue:** 测试在 `exporter.finalize()` 之后直接开新 rusqlite 连接，但由于 EXCLUSIVE locking mode pragma，连接在 exporter 整个生命期持有锁（不仅仅是事务期间），导致 "database is locked" 错误
- **Fix:** 用 `{ }` 代码块包裹 exporter，确保在验证查询之前 exporter drop（释放 EXCLUSIVE lock）。与项目中其他测试（如 `test_sqlite_basic_export`）的模式一致
- **Files modified:** src/exporter/sqlite.rs
- **Commit:** 0d27afb

**3. [Rule 2 - Missing] clippy doc-markdown 修复**
- **Found during:** Task 3 提交阶段
- **Issue:** 新增 helper 函数的文档注释中 `initialize()`、`SqliteFailure`、`Err` 未加 backtick，触发 `clippy::doc-markdown` 错误
- **Fix:** 为所有标识符添加 backtick（`` `initialize()` ``、`` `SqliteFailure` ``、`` `Err` ``、`` `log::warn!` ``）
- **Files modified:** src/exporter/sqlite.rs
- **Commit:** 0d27afb（在同一任务内修复）

## Self-Check: PASSED

| Item | Status |
|------|--------|
| src/config.rs 存在 | FOUND |
| src/exporter/sqlite.rs 存在 | FOUND |
| SUMMARY.md 存在 | FOUND |
| commit fb5532f 存在（Task 1） | FOUND |
| commit 0aeba09 存在（Task 2） | FOUND |
| commit 0d27afb 存在（Task 3） | FOUND |

---
phase: 05-sqlite
reviewed: 2026-05-10T10:00:00Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - benches/bench_sqlite.rs
  - src/cli/show_config.rs
  - src/config.rs
  - src/exporter/mod.rs
  - src/exporter/sqlite.rs
  - tests/integration.rs
findings:
  critical: 2
  warning: 4
  info: 3
  total: 9
status: issues_found
---

# Phase 05: Code Review Report

**Reviewed:** 2026-05-10T10:00:00Z
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

本次审查涵盖 SQLite 导出器（phase-05）新增的全部代码，包括 `SqliteExporter`、`batch_size` 配置字段、`show_config` 展示逻辑、基准测试与集成测试。

整体实现结构清晰，但存在两个 BLOCKER 级别缺陷：
1. `f32_ms_to_i64` 的溢出守卫逻辑错误（`i64::MAX` 不能精确表示为 `f64`），理论上可导致极端值时产生 UB/错误结果；
2. `DELETE FROM {table_name}` 静默吞掉错误，"无表则 DELETE" 场景下失去错误信号，且 `table_name` 没有经过标识符转义，存在 SQL 注入风险。

另有 4 处 WARNING 和 3 处 INFO。

---

## Critical Issues

### CR-01: `f32_ms_to_i64` 溢出守卫常量值错误，极端输入导致 UB

**File:** `src/exporter/mod.rs:332-336`

**Issue:**

```rust
const MAX_I64_F64: f64 = 9_223_372_036_854_775_807.0; // i64::MAX as f64
```

`i64::MAX`（= 2^63 - 1）无法精确表示为 `f64`；编译器会将该字面量向上取整为 `2^63`（= `9223372036854775808.0`）。

因此守卫比较变成 `ms_f64 > 2^63`：当 `ms` 是一个恰好等于 `2^63` 的 `f32`（该值可被 f32 精确表示）时，比较为 `2^63 > 2^63 = false`，代码进入 `else` 分支，执行 `2^63 as i64`，这在 Rust 中是**未定义行为**（debug 模式 panic，release 模式回绕到 `i64::MIN`）。

代码注释 `"value already clamped to i64 range"` 与实际情况不符。

虽然 `exec_time` 等于 2^63 ms（约 2.92 亿年）在日志中几乎不会出现，但守卫逻辑的错误注释会掩盖问题，且该函数声称提供正确的饱和转换行为。

**Fix:**

```rust
pub(super) fn f32_ms_to_i64(ms: f32) -> i64 {
    if !ms.is_finite() {
        return 0;
    }
    // 使用 (i64::MAX as f64) 向下取整得到的最大安全 f64 整数边界
    // i64::MAX = 2^63 - 1，不能精确表示为 f64，须用 < 而非 <=，或与 (i64::MAX as f64).floor() 比较
    const MAX_SAFE: f64 = 9_223_372_036_854_774_784.0; // 小于 i64::MAX 的最大 f64 整数
    let ms_f64 = f64::from(ms);
    if ms_f64 >= (i64::MAX as f64) {
        return i64::MAX;
    }
    if ms_f64 < (i64::MIN as f64) {
        return i64::MIN;
    }
    ms_f64.trunc() as i64
}
```

更简洁的方案（Rust 1.45+ 已稳定化饱和 float-to-int 转换）：

```rust
pub(super) fn f32_ms_to_i64(ms: f32) -> i64 {
    if !ms.is_finite() { return 0; }
    // saturating_cast: 溢出时饱和到 i64::MAX/MIN，NaN 转 0
    // 使用 f64 中间精度避免 f32 精度损失
    let ms_f64 = f64::from(ms);
    // Rust 不直接提供 f64::saturating_cast，手动实现：
    if ms_f64 >= (i64::MAX as f64) { i64::MAX }
    else if ms_f64 <= (i64::MIN as f64) { i64::MIN }
    else { ms_f64.trunc() as i64 }
}
```

注意：`i64::MAX as f64` 的结果是 2^63（比 i64::MAX 大 1），所以 `>=` 比较可正确将 2^63 饱和到 `i64::MAX`，而无需精确的常量值。

---

### CR-02: `table_name` 未转义即拼入 SQL，存在 SQL 注入风险

**File:** `src/exporter/sqlite.rs:260, 265, 76, 82, 112`

**Issue:**

多处直接将 `table_name` 字符串嵌入 SQL 语句：

```rust
// 第 260 行
conn.execute(&format!("DROP TABLE IF EXISTS {}", self.table_name), [])

// 第 265 行
let _ = conn.execute(&format!("DELETE FROM {}", self.table_name), []);

// 第 76、82、112 行（build_insert_sql / build_create_sql）
format!("INSERT INTO {table_name} VALUES (...)")
format!("CREATE TABLE IF NOT EXISTS {table_name} (...)")
```

`table_name` 来自用户配置（TOML 文件或 `--set` 命令行覆盖）。若用户指定 `table_name = "t; DROP TABLE other_table; --"` 这类值，可在 SQLite 中执行任意 DDL/DML。

虽然此工具的威胁模型是本地 CLI，攻击面有限，但 `validate()` 只检查 `table_name` 是否为空，没有验证其是否是合法的 SQL 标识符（无特殊字符、无分号等），这使注入理论上可行。

**Fix:**

在 `SqliteExporter::validate()` 中加入标识符合法性校验：

```rust
pub fn validate(&self) -> Result<()> {
    // ...已有检查...
    // 只允许合法 SQL 标识符：字母/数字/下划线，不含空白或特殊字符
    let valid_ident = self.table_name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid_ident || self.table_name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(Error::Config(ConfigError::InvalidValue {
            field: "exporter.sqlite.table_name".to_string(),
            value: self.table_name.clone(),
            reason: "table_name must be a valid SQL identifier (letters, digits, underscores only)".to_string(),
        }));
    }
    Ok(())
}
```

---

## Warnings

### WR-01: `DELETE FROM {table_name}` 错误被静默丢弃，表不存在时会漏报

**File:** `src/exporter/sqlite.rs:265`

**Issue:**

```rust
} else if !self.append {
    let _ = conn.execute(&format!("DELETE FROM {}", self.table_name), []);
}
```

使用 `let _ = ...` 静默忽略错误。当表不存在时（首次运行、非 `overwrite` 非 `append` 模式），DELETE 会返回错误，被丢弃后继续执行 `CREATE TABLE IF NOT EXISTS`，行为侥幸正确，但日志和错误统计均无记录，调试困难。

**Fix:**

```rust
} else if !self.append {
    // 表可能尚不存在（首次运行），忽略 "no such table" 错误
    if let Err(e) = conn.execute(&format!("DELETE FROM {}", self.table_name), []) {
        // 仅在非"表不存在"错误时上报
        if !e.to_string().contains("no such table") {
            return Err(Self::db_err(format!("clear table failed: {e}")));
        }
    }
}
```

---

### WR-02: `ExportStats.flush_operations` / `last_flush_size` 在 SQLite 路径中从不更新

**File:** `src/exporter/sqlite.rs:137-144`

**Issue:**

`ExportStats` 有 `flush_operations` 和 `last_flush_size` 两个字段，`log_stats()` 会打印这两个值（`mod.rs:295-300`）。`batch_commit_if_needed()` 是 SQLite 的批量提交入口，但其中从未更新这两个统计字段：

```rust
fn batch_commit_if_needed(&mut self) -> Result<()> {
    self.row_count += 1;
    if self.row_count % self.batch_size == 0 {
        let conn = self.conn.as_ref().unwrap();
        conn.execute_batch("COMMIT; BEGIN")
            .map_err(|e| Self::db_err(format!("batch commit failed: {e}")))?;
        // 未更新 stats.flush_operations 和 stats.last_flush_size！
    }
    Ok(())
}
```

结果：日志中 "flushed: N times" 这一行对 SQLite 导出器永远不出现，信息缺失。

**Fix:**

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

---

### WR-03: `show_config` 未展示 SQLite 的 `batch_size` 字段

**File:** `src/cli/show_config.rs:63-84`

**Issue:**

`handle_show_config` 的 SQLite 段落输出 `database_url`、`table_name`、`overwrite`、`append`，但遗漏了 `batch_size`，尽管它是 phase-05 新增的配置字段且直接影响性能：

```rust
if let Some(sqlite) = &cfg.exporter.sqlite {
    // ...
    kv("overwrite", &sqlite.overwrite.to_string(), def_ow, diff);
    kv("append", &sqlite.append.to_string(), def_ap, diff);
    // ← batch_size 缺失
    println!();
}
```

用户执行 `run show-config` 或 `run show-config --diff` 时看不到 `batch_size`，影响可观测性。

**Fix:**

在 `append` 之后添加：

```rust
let def_bs = def_sqlite.map(|d| d.batch_size.to_string());
kv("batch_size", &sqlite.batch_size.to_string(), def_bs.as_deref(), diff);
```

---

### WR-04: `PRAGMA page_size` 对已存在的数据库无效

**File:** `src/exporter/sqlite.rs:32`

**Issue:**

```
PRAGMA page_size = 65536;
```

SQLite 的 `page_size` PRAGMA 只在**数据库文件尚未创建**时生效；对于已存在的数据库文件，该 PRAGMA 静默无效。

在 `overwrite=true` 场景下（DROP TABLE + CREATE TABLE，但不重建数据库文件），数据库文件已经存在，`page_size=65536` 不会生效。若期望所有情况都使用 64KB 页，应先删除旧的 `.db` 文件再打开连接。

实际上注释和基准文档均未描述这一限制，导致代码的性能假设可能不成立（`overwrite` 第二次运行仍用默认 4KB 页）。

**Fix:**

在 `overwrite=true` 时，同时删除旧数据库文件：

```rust
if self.overwrite {
    // 删除旧文件使 page_size PRAGMA 生效
    if std::path::Path::new(&self.database_url).exists() {
        std::fs::remove_file(&self.database_url)
            .map_err(|e| Self::db_err(format!("remove old db failed: {e}")))?;
    }
}
let conn = Connection::open(&self.database_url)...;
```

或在文档中明确说明此限制。

---

## Info

### IN-01: `config.rs` 缺少 `batch_size=0` 验证的专项测试

**File:** `src/config.rs:404-411`

**Issue:**

`SqliteExporter::validate()` 有 `batch_size == 0` 的拒绝逻辑，但 `#[cfg(test)]` 模块中没有对应的测试用例。`test_default_sqlite_exporter_values` 也没有断言 `batch_size` 的默认值（10_000）。

**Fix:**

在 `config.rs` 的 tests 模块新增：

```rust
#[test]
fn test_validate_sqlite_batch_size_zero() {
    let mut cfg = default_config();
    cfg.exporter.csv = None;
    cfg.exporter.sqlite = Some(SqliteExporter {
        batch_size: 0,
        ..SqliteExporter::default()
    });
    assert!(cfg.validate().is_err());
}
```

---

### IN-02: 集成测试没有通过 `handle_run` 端到端验证 SQLite 导出路径

**File:** `tests/integration.rs`

**Issue:**

`tests/integration.rs` 中所有 `handle_run` 测试均使用 CSV 导出器。SQLite 导出器的集成路径（`handle_run` → `ExporterManager::from_config` → `SqliteExporter`）仅通过 `src/exporter/sqlite.rs` 的单元测试覆盖，没有端到端的集成测试验证完整流程（含配置加载、PRAGMA 设置、批量提交、最终 COMMIT）。

**Fix:**

在 `tests/integration.rs` 新增：

```rust
#[test]
fn test_handle_run_sqlite_export() {
    let dir = tempfile::TempDir::new().unwrap();
    let log_dir = dir.path().join("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    write_test_log(&log_dir.join("test.log"), 20);
    let db_file = dir.path().join("out.db");
    let cfg = Config {
        sqllog: SqllogConfig { path: log_dir.to_str().unwrap().to_string() },
        exporter: ExporterConfig {
            csv: None,
            sqlite: Some(SqliteExporter {
                database_url: db_file.to_str().unwrap().to_string(),
                table_name: "sqllog_records".to_string(),
                overwrite: true,
                append: false,
                batch_size: 10_000,
            }),
        },
        ..Default::default()
    };
    let interrupted = Arc::new(AtomicBool::new(false));
    handle_run(&cfg, None, false, true, &interrupted, 80, false, None, 1).unwrap();
    let conn = rusqlite::Connection::open(&db_file).unwrap();
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM sqllog_records", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 20);
}
```

---

### IN-03: `show_config` 用 `header.len()` 字节数计算分隔线，Unicode 路径下宽度不匹配

**File:** `src/cli/show_config.rs:9`

**Issue:**

```rust
let header = format!("Configuration ({config_path})");
println!("{}", color::dim("═".repeat(header.len())));
```

`String::len()` 返回字节数，而 `═`（U+2550）是 3 字节 UTF-8 字符，`repeat(header.len())` 会生成多于期望列数的分隔符。如果 `config_path` 含有中文字符（如 `/home/用户/config.toml`），字节数 > 字符数，分隔线也会偏长。

这是纯视觉问题，不影响正确性，但在 Unicode 路径下输出对齐错乱。

**Fix:**

```rust
let char_count = header.chars().count();
println!("{}", color::dim("═".repeat(char_count)));
```

（若需精确终端宽度，可用 `unicode-width` crate。）

---

_Reviewed: 2026-05-10T10:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

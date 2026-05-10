---
phase: 07-tech-debt-fix
reviewed: 2026-05-10T00:00:00Z
depth: standard
files_reviewed: 2
files_reviewed_list:
  - src/config.rs
  - src/exporter/sqlite.rs
findings:
  critical: 2
  warning: 3
  info: 3
  total: 8
status: issues_found
---

# Phase 07: Code Review Report

**Reviewed:** 2026-05-10
**Depth:** standard
**Files Reviewed:** 2
**Status:** issues_found

## Summary

审查了 `src/config.rs`（配置结构与验证逻辑）和 `src/exporter/sqlite.rs`（SQLite 导出器实现）。整体代码结构清晰，错误处理大致到位，测试覆盖率较高。但存在两个 Critical 级别问题：一是 SQL 插入快速路径与建表顺序不一致，可导致列数据静默错位；二是 `from_file` 将所有 IO 错误都映射为 `NotFound`，丢弃真实错误原因。此外有三个 Warning 级别问题：`overwrite+append` 同时启用无冲突校验、`DELETE FROM` 软失败可能导致静默数据追加、`finalize` 后连接未关闭导致接口契约模糊。

---

## Critical Issues

### CR-01: `build_insert_sql` 快速路径与 `do_insert_preparsed` 快速路径列顺序不一致，可导致数据静默列错位

**File:** `src/exporter/sqlite.rs:73-77` 及 `src/exporter/sqlite.rs:169-188`

**Issue:**
`build_insert_sql` 快速路径（`ordered_indices.len() == FIELD_NAMES.len()` 时）生成不含列名的 `INSERT INTO "t" VALUES (?, ...)` 语句，暗含"参数顺序与表列顺序一致"的假设。而 `build_create_sql` 按 `ordered_indices` 的顺序建表（第107-113行）。当 `ordered_indices` 包含全部15个字段但顺序不同于 `[0,1,...,14]` 时（例如用户在 `features.fields` 中指定了全部15个字段但重排了顺序），`build_create_sql` 会建立一张列顺序为用户自定义顺序的表，而 `do_insert_preparsed` 的 `FieldMask::ALL` 快速路径（第169行）始终按固定的 0-14 自然顺序绑定参数，导致数据列全部错位，且无任何错误提示。

触发路径：
1. 用户在 `features.fields` 配置中列出全部15个字段，但顺序重排（如最后放 `ts`）
2. `FeaturesConfig::field_mask()` 返回 `FieldMask::ALL`（因为所有15位都被置1）
3. `FeaturesConfig::ordered_field_indices()` 返回长度为15但顺序不同的索引
4. `build_create_sql` 以该顺序建表，`do_insert_preparsed` 以固定0-14顺序插入

**Fix:**
修改 `build_insert_sql` 快速路径的判断条件，改为同时检查长度相等且索引序列就是 `[0..14]`；或者删除快速路径，统一走显式列名路径：

```rust
fn build_insert_sql(table_name: &str, ordered_indices: &[usize]) -> String {
    use crate::features::FIELD_NAMES;
    // 删除快速路径，统一走带列名的 INSERT 语句，消除列顺序假设
    let cols: Vec<&str> = ordered_indices.iter().map(|&i| FIELD_NAMES[i]).collect();
    let placeholders = vec!["?"; ordered_indices.len()].join(", ");
    format!(
        "INSERT INTO \"{table_name}\" ({}) VALUES ({placeholders})",
        cols.join(", ")
    )
}
```

同时需要保证 `do_insert_preparsed` 的全量掩码路径也要么使用固定顺序（配合带列名的 INSERT），要么按 `ordered_indices` 顺序绑定参数。推荐后者：移除 `FieldMask::ALL` 快速路径中的顺序固定绑定，改为和投影路径统一走 `ordered_indices` 驱动的 `all[i]` 选取。

---

### CR-02: `Config::from_file` 将所有 IO 错误映射为 `NotFound`，丢失真实错误原因

**File:** `src/config.rs:44-45`

**Issue:**
```rust
let content = std::fs::read_to_string(path)
    .map_err(|_| Error::Config(ConfigError::NotFound(path.to_path_buf())))?;
```

`map_err(|_|)` 忽略了原始 `std::io::Error`。当文件存在但权限被拒绝（`EACCES`）、路径是目录（`EISDIR`）、或路径包含无效 Unicode 等情况时，用户会收到"配置文件未找到"的误导性错误，而实际问题完全不同，导致用户难以排查。

**Fix:**
保留原始 IO 错误信息，根据错误类型区分 `NotFound` 和其他 IO 错误：

```rust
let content = std::fs::read_to_string(path).map_err(|e| {
    if e.kind() == std::io::ErrorKind::NotFound {
        Error::Config(ConfigError::NotFound(path.to_path_buf()))
    } else {
        Error::Config(ConfigError::ParseFailed {
            path: path.to_path_buf(),
            reason: format!("failed to read file: {e}"),
        })
    }
})?;
```

或者直接在 `ConfigError` 中增加 `ReadFailed` 变体（与 `FileError::ReadFailed` 对应），更语义准确。

---

## Warnings

### WR-01: `overwrite=true` 与 `append=true` 同时设置无冲突校验，语义未定义

**File:** `src/config.rs:343-352`（`CsvExporter::validate`）及 `src/config.rs:389-429`（`SqliteExporter::validate`）

**Issue:**
`CsvExporter` 和 `SqliteExporter` 的 `validate()` 均未检查 `overwrite` 和 `append` 同时为 `true` 的情况。在 `SqliteExporter::prepare_target_table`（`sqlite.rs:235`），当两者同时为 true 时，`overwrite` 分支优先，表会被 DROP 重建，`append` 被静默忽略——用户意图不明，但不会报错。这可能掩盖配置错误，且 CSV 导出器的行为可能不同。

**Fix:**
在 `validate()` 中加入互斥检查：

```rust
// CsvExporter::validate() 末尾 / SqliteExporter::validate() 末尾增加
if self.overwrite && self.append {
    return Err(Error::Config(ConfigError::InvalidValue {
        field: "overwrite/append".to_string(),
        value: "both true".to_string(),
        reason: "overwrite and append are mutually exclusive".to_string(),
    }));
}
```

---

### WR-02: `DELETE FROM` 软失败可能导致静默数据追加，违背截断模式语义

**File:** `src/exporter/sqlite.rs:222-250`

**Issue:**
当 `overwrite=false, append=false`（截断重写模式）时，`prepare_target_table` 调用 `DELETE FROM` 清空已有数据，但通过 `handle_delete_clear_result` 以"软失败"方式处理错误——非 `no such table` 的错误只记录 `log::warn!` 后继续执行。如果 `DELETE FROM` 因磁盘满、权限问题或其他原因失败，旧数据未被清除，随后的 INSERT 将把新数据追加到旧数据之后，用户毫不知情，且 `validate()` 和 `finalize()` 均不会报告该问题。

**Fix:**
将 `DELETE FROM` 失败（除 `no such table` 外）改为硬错误向上传播：

```rust
fn prepare_target_table(&self) -> Result<()> {
    if self.overwrite {
        // ... 保持不变
    } else if !self.append {
        let conn = self.conn.as_ref().unwrap();
        match conn.execute(&format!("DELETE FROM \"{}\"", self.table_name), []) {
            Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(_, Some(ref msg))) if msg.contains("no such table") => {
                // 首次运行，表不存在，正常
            }
            Err(e) => {
                return Err(Self::db_err(format!("clear table failed: {e}")));
            }
        }
    }
    Ok(())
}
```

---

### WR-03: `finalize()` 后未将 `self.conn` 置 `None`，`export()` 可在 finalize 后继续调用

**File:** `src/exporter/sqlite.rs:392-402`

**Issue:**
`finalize()` 执行 `COMMIT` 但不关闭连接（未设置 `self.conn = None`）。之后若外部代码再调用 `export()`，由于连接仍然有效，`export()` 会成功获取连接并插入数据，但此时没有活跃的 `BEGIN TRANSACTION`（已在 `finalize()` 中被 `COMMIT` 关闭），SQLite 将为每条 INSERT 开启隐式单条事务，性能骤降，且超出了 `Exporter` 接口的设计契约（`finalize` 意为"完成写入"）。目前没有 `Drop` 实现来主动关闭连接或报告未 finalize 的状态。

**Fix:**
在 `finalize()` 中关闭连接并清除状态：

```rust
fn finalize(&mut self) -> Result<()> {
    if let Some(conn) = self.conn.take() {  // take() 消费连接，Drop 时自动关闭
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

使用 `take()` 后，连接在 `finalize()` 调用点完成 COMMIT 后立即 Drop（释放 EXCLUSIVE 锁），并且后续 `export()` 调用会命中 `ok_or_else(|| Self::db_err("not initialized"))` 返回错误。

---

## Info

### IN-01: `PRAGMA page_size` 设置位置脆弱，对已存在数据库文件静默无效

**File:** `src/exporter/sqlite.rs:32`

**Issue:**
`PRAGMA page_size = 65536` 在 `Connection::open()` 之后、第一次写操作之前设置，仅对**新建**数据库有效。对已存在的数据库文件，SQLite 会静默忽略该 PRAGMA，实际 page_size 保持旧值。当前代码顺序恰好满足对新文件的要求，但若未来调整 pragma 设置与建表顺序，可能静默失效。建议在注释中标注此限制。

**Fix:**
在 `initialize_pragmas` 函数或调用处添加注释说明：

```rust
// NOTE: PRAGMA page_size must be set before the first write operation.
// For existing databases, this pragma is silently ignored by SQLite.
"PRAGMA page_size = 65536;"
```

---

### IN-02: `apply_overrides` 中 `logging.retention_days` 缺少范围校验，与 `validate()` 行为不一致

**File:** `src/config.rs:120-128`

**Issue:**
`apply_overrides` 中设置 `logging.retention_days` 时仅验证能否解析为 `usize`，允许设置 `0` 或 `>365` 的值（这些在 `validate()` 中会失败）。这意味着调用 `apply_overrides(&["logging.retention_days=0"])` 不报错，但随后调用 `validate()` 才报错。这种两步陷阱可能造成调用顺序依赖的混乱，建议在 `apply_overrides` 中执行与 `validate()` 相同的范围检查，或在文档/错误消息中明确说明需要配合 `validate()` 使用。

**Fix:**
在 `apply_overrides` 的 `retention_days` 分支中加入范围检查：

```rust
"logging.retention_days" => {
    let parsed: usize = value.parse().map_err(|_| { /* ... */ })?;
    if parsed == 0 || parsed > 365 {
        return Err(Error::Config(ConfigError::InvalidValue {
            field: key.to_string(),
            value: value.to_string(),
            reason: "retention days must be between 1 and 365".to_string(),
        }));
    }
    self.logging.retention_days = parsed;
}
```

---

### IN-03: `build_create_sql` / `build_insert_sql` 中 `ordered_indices` 未进行越界检查，越界时 panic

**File:** `src/exporter/sqlite.rs:109` 及 `src/exporter/sqlite.rs:79`

**Issue:**
`build_create_sql` 中 `FIELD_NAMES[i]` 和 `COL_TYPES[i]`、`build_insert_sql` 中 `FIELD_NAMES[i]` 均未校验 `i < 15`。当 `ordered_indices` 包含 `>= 15` 的索引时，代码会 panic。目前通过 `FeaturesConfig::ordered_field_indices()` 生成的索引不会越界，但 `SqliteExporter::ordered_indices` 是 `pub(super)` 字段，测试代码（如 `test_sqlite_field_order`）会直接赋值，未来可能误传越界值。

**Fix:**
在 `initialize()` 中调用 `build_create_sql` 前加断言，或在函数内部添加检查：

```rust
fn build_create_sql(table_name: &str, ordered_indices: &[usize]) -> String {
    use crate::features::FIELD_NAMES;
    assert!(
        ordered_indices.iter().all(|&i| i < FIELD_NAMES.len()),
        "ordered_indices contains out-of-range index"
    );
    // ...
}
```

---

_Reviewed: 2026-05-10_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

# Phase 7: 技术债修复 - Pattern Map

**Mapped:** 2026-05-10
**Files analyzed:** 2 (src/exporter/sqlite.rs, src/config.rs)
**Analogs found:** 2 / 2

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/exporter/sqlite.rs` | exporter / service | CRUD, file-I/O | `src/exporter/sqlite.rs` (self — in-place edit) | exact |
| `src/config.rs` | config / validation | request-response | `src/config.rs` `LoggingConfig::validate()` + `CsvExporter::validate()` | exact |

---

## Pattern Assignments

### `src/exporter/sqlite.rs` — DEBT-01: 静默错误修复

**修改点：** `initialize()` 方法，第 265 行 `let _ = conn.execute(...)` 改为显式 `match`。

**现有致命错误传播模式**（`sqlite.rs` lines 250–253, 260–262）:
```rust
let conn = Connection::open(&self.database_url)
    .map_err(|e| Self::db_err(format!("open failed: {e}")))?;

initialize_pragmas(&conn).map_err(|e| Self::db_err(format!("set PRAGMAs failed: {e}")))?;

conn.execute(&format!("DROP TABLE IF EXISTS {}", self.table_name), [])
    .map_err(|e| Self::db_err(format!("drop table failed: {e}")))?;
```

**静默丢弃（现有问题，line 265）:**
```rust
// DEBT-01 修复前（当前代码）
let _ = conn.execute(&format!("DELETE FROM {}", self.table_name), []);
```

**修复后应替换为的 match 模式（参考 rusqlite Error 结构）:**
```rust
// DEBT-01 修复后
match conn.execute(&format!("DELETE FROM \"{}\"", self.table_name), []) {
    Ok(_) => {}
    Err(rusqlite::Error::SqliteFailure(_, Some(ref msg))) if msg.contains("no such table") => {
        // 首次运行，表尚不存在，完全静默
    }
    Err(e) => {
        log::warn!("sqlite clear failed: {e}");
        // 软失败：warn 后继续，不中止 initialize()
    }
}
```

**注意：** `log::warn!` 在 `sqlite.rs` 已通过 `use log::info;` 所在的 `log` crate 可用，
只需将 `info` 改为同 crate 下的 `warn`，无需新增 import（`log::warn!` 宏路径已被 `log` 解析）。

---

### `src/exporter/sqlite.rs` — DEBT-02: DDL 双引号标识符转义

**修改点：** `build_insert_sql()` (lines 71–85)、`build_create_sql()` (lines 88–115)、
`initialize()` 中的 `DROP TABLE IF EXISTS` (line 260) 和 `DELETE FROM` (line 265)，
以及 `new()` 中的 `insert_sql` 初始化 (lines 51–53)。

**现有未转义 DDL（当前代码，共 4 处）:**
```rust
// new() line 51–53
format!("INSERT INTO {table_name} VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")

// build_insert_sql() line 75–77
format!("INSERT INTO {table_name} VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")

// build_insert_sql() line 82–84
format!("INSERT INTO {table_name} ({}) VALUES ({placeholders})", cols.join(", "))

// build_create_sql() line 111–114
format!("CREATE TABLE IF NOT EXISTS {table_name} ({})", cols.join(", "))

// initialize() line 260
format!("DROP TABLE IF EXISTS {}", self.table_name)

// initialize() line 265（DEBT-01 同步修改）
format!("DELETE FROM {}", self.table_name)
```

**修复后的转义模式（全部 4 条 DDL）:**
```rust
// new() — 使用双引号转义 table_name
format!("INSERT INTO \"{table_name}\" VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")

// build_insert_sql() 全量快速路径
format!("INSERT INTO \"{table_name}\" VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")

// build_insert_sql() 投影路径
format!("INSERT INTO \"{}\" ({}) VALUES ({placeholders})", table_name, cols.join(", "))

// build_create_sql()
format!("CREATE TABLE IF NOT EXISTS \"{}\" ({})", table_name, cols.join(", "))

// initialize() DROP TABLE
format!("DROP TABLE IF EXISTS \"{}\"", self.table_name)

// initialize() DELETE FROM（含 DEBT-01 的 match 结构）
format!("DELETE FROM \"{}\"", self.table_name)
```

---

### `src/config.rs` — DEBT-02: table_name ASCII 标识符校验

**修改点：** `SqliteExporter::validate()` (lines 389–413)，在现有 `table_name` 非空检查之后追加格式校验。

**现有 validate() 模式**（`config.rs` lines 389–413）:
```rust
impl SqliteExporter {
    pub fn validate(&self) -> Result<()> {
        if self.database_url.trim().is_empty() {
            return Err(Error::Config(ConfigError::InvalidValue {
                field: "exporter.sqlite.database_url".to_string(),
                value: self.database_url.clone(),
                reason: "SQLite database URL cannot be empty".to_string(),
            }));
        }
        if self.table_name.trim().is_empty() {
            return Err(Error::Config(ConfigError::InvalidValue {
                field: "exporter.sqlite.table_name".to_string(),
                value: self.table_name.clone(),
                reason: "SQLite table name cannot be empty".to_string(),
            }));
        }
        if self.batch_size == 0 {
            return Err(ConfigError::InvalidValue {
                field: "exporter.sqlite.batch_size".to_string(),
                value: "0".to_string(),
                reason: "batch_size must be greater than 0".to_string(),
            }
            .into());
        }
        Ok(())
    }
}
```

**追加的 ASCII 标识符校验（插入在 table_name 非空检查之后，batch_size 检查之前）:**
```rust
// ASCII 标识符校验：^[a-zA-Z_][a-zA-Z0-9_]*$（不使用 regex crate，纯 char 方法）
let is_valid_ident = {
    let mut chars = self.table_name.chars();
    chars.next().map_or(false, |c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
};
if !is_valid_ident {
    return Err(Error::Config(ConfigError::InvalidValue {
        field: "exporter.sqlite.table_name".to_string(),
        value: self.table_name.clone(),
        reason: "table name must match ^[a-zA-Z_][a-zA-Z0-9_]*$ (ASCII identifiers only)".to_string(),
    }));
}
```

**注意：** 不引入 `regex` crate，用 `str::chars()` + `char::is_ascii_*()` 方法实现，
与项目简洁风格一致（`LoggingConfig::validate()` 中的 `eq_ignore_ascii_case` 是同类模式）。

---

## Shared Patterns

### ConfigError::InvalidValue 构造模式
**Source:** `src/error.rs` lines 55–59, `src/config.rs` lines 391–395
**Apply to:** `SqliteExporter::validate()` 中新增的 table_name 格式校验
```rust
return Err(Error::Config(ConfigError::InvalidValue {
    field: "exporter.sqlite.<field>".to_string(),
    value: self.<field>.clone(),
    reason: "<具体原因>".to_string(),
}));
```

### log::warn! 软失败模式
**Source:** `src/exporter/sqlite.rs` line 1 (`use log::info;`)
**Apply to:** DEBT-01 的 DELETE FROM 错误处理
```rust
// 引入方式：无需新增 use，log::warn! 与 log::info! 同宏路径
log::warn!("sqlite clear failed: {e}");
```

### map_err + db_err 致命错误传播模式
**Source:** `src/exporter/sqlite.rs` lines 250–262
**Apply to:** DEBT-01 中不改变的其他 initialize() 分支（保持现有 ? 传播不受影响）
```rust
.map_err(|e| Self::db_err(format!("<operation> failed: {e}")))?;
```

---

## No Analog Found

无。所有修改文件在项目中均有直接实现可参考。

---

## Metadata

**Analog search scope:** `src/exporter/`, `src/config.rs`, `src/error.rs`, `src/features/filters.rs`
**Files scanned:** 4 source files
**Pattern extraction date:** 2026-05-10

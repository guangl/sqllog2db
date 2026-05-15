# Phase 7: 技术债修复 - Context

**Gathered:** 2026-05-10
**Status:** Ready for planning

<domain>
## Phase Boundary

修复 SQLite 导出器的两处已知技术债：

1. **DEBT-01** — `initialize()` 中 `DELETE FROM` 的静默错误（`let _ = ...`）改为可观测：区分无害的"表不存在"与真实错误，真实错误通过 `log::warn!()` 记录后继续运行。
2. **DEBT-02** — `table_name` 直接拼接进 SQL 字符串的注入风险：在 `validate()` 添加 ASCII 标识符校验，所有 4 条 DDL 使用 SQLite 双引号标识符转义。

不涉及新功能、不改变 CSV 路径、不修改 ExporterManager 或 Pipeline 结构。

</domain>

<decisions>
## Implementation Decisions

### DEBT-01：静默错误修复

- **D-01:** `let _ = conn.execute("DELETE FROM ...", [])` 改为显式 `match`：
  - 匹配 `rusqlite::Error::SqliteFailure` 且错误信息含 `"no such table"` → 完全静默（正常首次运行）
  - 其他任何错误 → `log::warn!("sqlite clear failed: {e}")` 后继续运行（软失败，不中止）
- **D-02:** 错误输出目标为**应用日志**（`[logging] file`，即 `logs/sqllog2db.log`），通过 `log::warn!()` 写入。**不**修改 `SqliteExporter` 结构体，不传入 error writer 句柄——最小侵入性。
- **D-03:** `initialize()` 继续向上传播真正致命的错误（如 DB 打开失败、PRAGMA 失败、CREATE TABLE 失败）——这些已经是 `?` 传播，不受影响。

### DEBT-02：SQL 注入防护

- **D-04:** 在 `config.rs` 的 `SqliteExporter::validate()` 中添加 `table_name` 校验：
  - 合法规则：`^[a-zA-Z_][a-zA-Z0-9_]*$`（严格 ASCII 标识符，起头必须是字母或下划线）
  - 非法时返回 `Err(ConfigError::InvalidValue { field: "exporter.sqlite.table_name", ... })`
  - `cargo run -- validate` 会调用此路径，用户会得到明确错误信息并拒绝启动
- **D-05:** 所有 4 条 DDL 语句中的 `table_name` 改为 SQLite 双引号标识符转义：
  - `format!("DROP TABLE IF EXISTS \"{}\"", self.table_name)` 
  - `format!("DELETE FROM \"{}\"", self.table_name)`
  - `build_create_sql` 内 `format!("CREATE TABLE IF NOT EXISTS \"{}\" (...)", table_name)`
  - `build_insert_sql` 内 `format!("INSERT INTO \"{}\" ...", table_name)`
- **D-06:** 双引号本身在 SQLite 标识符中需要转义为 `""`（两个双引号）。但由于 D-04 已拒绝包含引号的 table_name，运行时无需额外转义——validate 层面保证安全。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 核心实现文件
- `src/exporter/sqlite.rs` L260–285 — `initialize()` 方法，含 `let _ = conn.execute(DELETE FROM ...)` 静默错误（DEBT-01 修复点）
- `src/exporter/sqlite.rs` L71–120 — `build_insert_sql()` 和 `build_create_sql()`（DEBT-02 双引号转义点）
- `src/config.rs` L391–412 — `SqliteExporter::validate()`，需添加 `table_name` 格式校验

### 错误处理 & 配置
- `src/error.rs` — `ConfigError::InvalidValue` 结构，用于 validate() 返回的错误类型
- `config.toml` — `[error]` 和 `[logging]` 区块，确认错误输出路径

### 成功标准参考
- `.planning/ROADMAP.md` Phase 7 Success Criteria（4 条）— 验收判断依据，planner 必读

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SqliteExporter::validate()` in `config.rs` — 已有 `database_url`、`table_name`（空校验）、`batch_size` 检查，直接在此追加 ASCII 标识符 regex 校验
- `ConfigError::InvalidValue` — 已有的错误变体，直接复用，无需新增错误类型
- `log::warn!()` macro — 已在 `sqlite.rs` 导入，直接使用

### Established Patterns
- `map_err(|e| Self::db_err(format!(...)))?` — 现有 initialize() 的致命错误传播模式；DEBT-01 的 warn 路径不使用此模式（warn + 继续）
- `let _ = ...` 模式在项目中仅此一处（sqlite.rs:265），修复后全部消除
- rusqlite `SqliteFailure { code, .. }` 匹配方式 — 需确认 rusqlite API，extended_code 或 message 字段识别 "no such table"

### Integration Points
- `validate()` 在 `cli/run.rs` 的 `validate` 子命令路径中被调用——新增校验自动生效
- `initialize()` 由 `ExporterManager` 调用，返回 `Result<()>`——warn 后不返回 Err 意味着 initialize 仍视为成功

</code_context>

<specifics>
## Specific Ideas

- "no such table" 判断：优先检查 `rusqlite::Error::SqliteFailure` 内的 error message 是否包含 `"no such table"`（比匹配 extended_code 更直观，避免依赖 SQLite 内部错误码常量）
- 双引号转义后 SQL 示例：`DROP TABLE IF EXISTS "sqllog_records"` — SQLite 标准合规，无需 prepare 参数化

</specifics>

<deferred>
## Deferred Ideas

None — 讨论完全在 Phase 7 范围内进行，无超出范围的提案。

</deferred>

---

*Phase: 7-技术债修复*
*Context gathered: 2026-05-10*

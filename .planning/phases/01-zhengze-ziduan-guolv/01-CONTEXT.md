# Phase 1: 正则字段过滤 - Context

**Gathered:** 2026-04-18
**Status:** Ready for planning

<domain>
## Phase Boundary

为 `MetaFilters` 的所有元数据字段（usernames, client_ips, appnames, tags, sess_ids, thrd_ids, statements）以及 `record_sql`（记录级 SQL 过滤）添加正则表达式匹配能力，并将多字段组合语义从 OR 改为 AND。

**不在本阶段范围内**：
- 事务级 sql 的正则过滤（预扫描层，属于未来扩展）
- FILTER-03 排除模式（已在 Future Requirements）
- 字段投影控制（Phase 2）

</domain>

<decisions>
## Implementation Decisions

### Config 格式
- **D-01:** 升级现有字段支持正则。`usernames`, `client_ips`, `appnames`, `tags`, `sess_ids`, `thrd_ids`, `statements` 等字段直接接受正则字符串，配置格式不变。纯字符串本身就是合法的正则，向后兼容——现有配置无需修改。

### 字段内语义（同字段多值）
- **D-02:** 同一字段的列表内多个正则是 **OR** 语义——任意一个正则匹配即满足该字段的过滤条件（类似"允许名单"：`usernames = ["^admin.*", ".*_dba"]` 表示 admin 开头或 _dba 结尾的用户名）。

### 跨字段语义（多字段组合）
- **D-04:** 跨字段是 **AND** 语义——所有配置了过滤条件的字段必须同时满足，记录才被保留。例如同时配置 `usernames` 和 `client_ips`，则用户名和 IP 两个条件都必须通过。这会替换现有 `should_keep()` 中跨字段 OR 的逻辑。时间范围（start_ts/end_ts）依然是独立的 AND 前置条件。

### SQL 过滤层级
- **D-03:** `record_sql` 的 `include_patterns`/`exclude_patterns` 字段升级为正则匹配（原子串包含匹配），在记录级主循环中判断。事务级 `sql` 过滤（预扫描）保持现有字符串包含匹配，不在本阶段升级。

### 正则编译时机
- **Claude's Discretion:** 正则在配置加载后、进入热循环前编译（`FiltersFeature::from_config()` 或类似构造时），启动阶段报错而非运行时。实现方式（`regex::Regex` 直接编译 vs `regex::RegexSet`）由 Claude 决定。

### 无过滤快路径
- 沿用现有设计：未配置任何过滤条件时 `pipeline.is_empty()` 快路径保持零开销，无性能损耗。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

No external specs — requirements fully captured in decisions above.

### 核心源文件
- `src/features/filters.rs` — FiltersFeature, MetaFilters, SqlFilters 的当前实现（需修改）
- `src/features/mod.rs` — FeaturesConfig, FieldMask, Pipeline, LogProcessor trait
- `src/cli/run.rs` — FilterProcessor 实现，热循环逻辑（process_with_meta）
- `src/config.rs` — Config 结构，validate() 方法（需添加正则验证）
- `.planning/REQUIREMENTS.md` — FILTER-01, FILTER-02 的验收标准
- `.planning/PROJECT.md` — 项目约束（性能、配置兼容性）

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `regex` crate: 需在 Cargo.toml 中确认是否已引入（当前 SqlFilters 用字符串包含，未使用 regex）
- `MetaFilters::match_substring()`: 现有字段匹配辅助函数，将被替换为正则匹配
- `FilterProcessor::has_meta_filters`: 预计算布尔值，避免热循环中重复扫描 Option 字段

### Established Patterns
- 正则需要在构造时预编译（类似 `TrxidSet` 在反序列化时预处理为 HashSet）
- 错误处理遵循 `ConfigError::InvalidValue { field, value, reason }` 模式
- 热路径方法标注 `#[inline]`
- Config 验证在 `Config::validate()` 中集中处理

### Integration Points
- `FiltersFeature::should_keep()` 和 `MetaFilters::should_keep()` 是语义改动的核心——需将跨字段 OR 改为 AND
- `FilterProcessor::new()` 预计算 `has_meta_filters`，修改后需同步更新
- `Config::validate()` 需新增正则格式验证逻辑
- `SqlFilters::matches()` 中字符串 `.contains()` 需升级为 `regex::Regex::is_match()`

</code_context>

<specifics>
## Specific Ideas

- 正则字段的向后兼容：纯字符串如 `"SYSDBA"` 作为正则与作为子串匹配行为基本一致（字符串是特殊正则），用户现有配置无需修改
- 跨字段 AND 是 FILTER-02 的核心价值：用户可以精确定位"特定用户 + 特定 IP + 特定 SQL 关键词"的记录组合

</specifics>

<deferred>
## Deferred Ideas

- 事务级 `sql` 过滤（预扫描）的正则升级 — 留到后续，当前保持字符串包含
- FILTER-03 排除模式 — 已在 Future Requirements
- OR 条件组合 — 明确在 Out of Scope

</deferred>

---

*Phase: 01-zhengze-ziduan-guolv*
*Context gathered: 2026-04-18*

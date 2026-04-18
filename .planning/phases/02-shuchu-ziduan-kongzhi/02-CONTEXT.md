# Phase 2: 输出字段控制 - Context

**Gathered:** 2026-04-18
**Status:** Ready for planning

<domain>
## Phase Boundary

用户可在 `config.toml` 中通过 `features.fields = [...]` 指定导出哪些字段及其列顺序。未指定则导出全部 15 个字段，行为与现有版本完全一致。字段名不合法时启动阶段报错（已实现）。

</domain>

<decisions>
## Implementation Decisions

### 列顺序语义
- **D-01:** 列顺序按用户配置顺序输出（而非固定原始顺序）。需在 FieldMask bitmask 之外额外存储 `Vec<usize>` 有序字段索引列表，供 exporter 按顺序写入列。

### 空列表处理
- **D-02:** `features.fields = []` 等同于不配置（导出全部字段），不报错，零歧义。

### normalized_sql 联动
- **D-03:** fields 列表中未包含 `normalized_sql` 时，即使 `replace_parameters.enable = true`，也静默忽略（不导出 normalized_sql，不给出警告）。replace_parameters 功能照常执行，结果在写入阶段丢弃。

### Claude's Discretion
- FieldMask 与有序索引的具体数据结构（`Vec<usize>` 还是其他形式）由实现决定，需确保与现有 FieldMask API 向后兼容。
- CSV header 行和 SQLite 建表语句中的列名顺序均跟随有序索引列表。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 现有代码（必读）
- `src/features/mod.rs` — FieldMask、FIELD_NAMES（15 个字段定义）、FeaturesConfig::fields 和 field_mask() 方法（已实现，需扩展为有序版本）
- `src/config.rs` — Config::validate() 字段名校验（已实现，无需修改）
- `src/exporter/csv.rs` — CsvExporter 写入逻辑（需接线有序字段索引）
- `src/exporter/sqlite.rs` — SqliteExporter 建表和写入逻辑（需接线有序字段索引）
- `src/cli/run.rs` — handle_run 函数，exporter 初始化点（需传递有序字段索引）

### 需求文档
- `.planning/REQUIREMENTS.md` §FIELD-01 — 输出字段控制需求定义

</canonical_refs>

<code_context>
## Existing Code Insights

### 已实现（无需重新实现）
- `FeaturesConfig.fields: Option<Vec<String>>` — 配置字段已存在，serde 已接线
- `FieldMask::from_names(names)` — 从字段名列表构建 bitmask（已实现）
- `FeaturesConfig::field_mask()` — 计算 FieldMask（已实现）
- `Config::validate()` 中的字段名合法性校验 — 未知字段名启动报错（已实现）
- `FIELD_NAMES: &[&str]` — 15 个字段名，顺序即原始列定义顺序

### 需要扩展/新增
- `FieldMask` 或新增 `OrderedFieldMask` — 需要保存有序字段索引 `Vec<usize>` 以支持 D-01
- `FeaturesConfig::ordered_field_indices()` — 从 `fields: Vec<String>` 计算有序索引列表
- CSV exporter — 按有序索引列表写入 header 和每行数据
- SQLite exporter — 按有序索引列表建表（CREATE TABLE 列顺序）和写入行

### 集成点
- `handle_run` 计算 `ordered_indices` 后传入 exporter 初始化
- `pipeline.is_empty()` 快路径不受影响（字段投影在 exporter 层处理，不在 pipeline）

</code_context>

<specifics>
## Specific Ideas

- 无需改动 Pipeline 和 FilterProcessor——字段投影在 exporter 写入时处理，不在过滤阶段
- `features.fields` 未配置 → 有序索引 = [0,1,2,...,14]（原始顺序，所有字段）
- `features.fields = []` → 同上（D-02 决策）

</specifics>

<deferred>
## Deferred Ideas

None — 讨论严格在 Phase 2 范围内进行。

</deferred>

---

*Phase: 02-shuchu-ziduan-kongzhi*
*Context gathered: 2026-04-18*

# Phase 8: 排除过滤器 - Context

**Gathered:** 2026-05-10
**Status:** Ready for planning

<domain>
## Phase Boundary

为 `MetaFilters` 的 7 个元数据字段（username / client_ip / sess_id / thrd_id / statement / appname / tag）新增"匹配则丢弃"的排除规则。配置键以 `exclude_` 前缀平铺在 `[features.filters]` 下。语义为 OR veto：任一 exclude 字段的正则命中即丢弃该记录，排除检查在包含检查之前执行（短路更快）。未配置任何 exclude 字段时，`pipeline.is_empty()` 快路径完全不受影响。

不涉及：`exclude_trxids`（明确移出范围，与 include_trxids 的 HashSet 精确匹配对称性保留）、时间戳排除、SQL 级别的排除（已有 `record_sql.exclude_patterns`）。

</domain>

<decisions>
## Implementation Decisions

### TOML 配置结构

- **D-01:** exclude 字段**平铺**在 `[features.filters]` 下，键名以 `exclude_` 前缀 + **复数**形式，与现有 include 字段对称：
  - `exclude_usernames = ["pattern"]`
  - `exclude_client_ips = ["pattern"]`
  - `exclude_sess_ids = ["pattern"]`
  - `exclude_thrd_ids = ["pattern"]`
  - `exclude_statements = ["pattern"]`
  - `exclude_appnames = ["pattern"]`
  - `exclude_tags = ["pattern"]`
- **D-02:** exclude 字段加入 `MetaFilters` struct（与 include 字段并排，`serde` 自动 deserialize），类型为 `Option<Vec<String>>`。

### 编译与热路径架构

- **D-03:** 编译后的 exclude 正则**扩展 `CompiledMetaFilters` 结构体**，新增 `exclude_usernames: Option<Vec<Regex>>` 等 7 个字段。不建独立 struct。
- **D-04:** `CompiledMetaFilters::should_keep()` 重构为先执行 **exclude 检查**（任一命中 → 直接返回 `false`），再执行 include 检查。短路语义，热路径最快。
- **D-05:** `FilterProcessor::has_meta_filters` 预计算**包含** exclude 字段——`compiled_meta.has_filters_any()`（include 或 exclude 任一非空即 `true`）。确保纯 exclude 配置也正确激活 meta 检查路径。

### has_filters() 扩展

- **D-06:** `MetaFilters::has_filters()` 扩展：同时检查 include 字段和 exclude 字段，任一非空则返回 `true`。
  这样 `FiltersFeature::has_filters()` → `self.meta.has_filters()` 链条自动覆盖纯 exclude 配置场景。
- **D-07:** 空配置（所有 exclude_* 字段均未设置）时，`has_filters()` 仍返回 `false`，`pipeline.is_empty()` 返回 `true`，零额外开销。

### validate_regexes() 扩展

- **D-08:** `FiltersFeature::validate_regexes()` 追加对 7 个 `exclude_*` 字段的正则校验。非法 exclude 正则在 `cargo run -- validate` 阶段报 `ConfigError::InvalidValue`，不推迟到运行时。

### init 模板更新

- **D-09:** `cargo run -- init` 生成的 config.toml 在 meta filters 区域，**每个 include 字段注释下方**紧跟对应的 exclude 注释示例：
  ```toml
  # Filter by usernames (regex match)
  # usernames = ["^SYSDBA"]
  # Exclude by usernames (OR veto: any match drops the record)
  # exclude_usernames = ["guest", "^anon"]
  ```
  verbose 模板和 minimal 模板均更新。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 核心过滤实现文件
- `src/features/filters.rs` — `MetaFilters`（include 字段定义）、`CompiledMetaFilters`（热路径 AND 语义，should_keep()）、`compile_patterns()`、`validate_pattern_list()`、`FiltersFeature::validate_regexes()`——所有扩展点均在此文件
- `src/cli/run.rs` L21–110 — `build_pipeline()`、`FilterProcessor` 结构体及 `process_with_meta()`——热路径调用链，has_meta_filters 预计算，需同步扩展

### 配置结构
- `src/cli/init.rs` L160–200 — meta filters 注释示例区域，需在每个 include 字段下方插入对应 exclude 注释

### 成功标准参考
- `.planning/ROADMAP.md` Phase 8 Success Criteria（5 条）——验收判断依据，planner 必读
- `.planning/REQUIREMENTS.md` FILTER-03——需求描述，含 Out of Scope（exclude_trxids）

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `compile_patterns(patterns: Option<&[String]>) -> Result<Option<Vec<Regex>>, String>` (`filters.rs` L251–266) — 直接复用，无需修改，用于编译 7 个 exclude_* 字段
- `validate_pattern_list(field: &str, patterns: Option<&[String]>)` (`filters.rs` L278–292) — 直接复用，追加 7 次调用即可完成 exclude 正则校验
- `match_any_regex(patterns: Option<&[Regex]>, val: &str) -> bool` (`filters.rs` L269–275) — exclude 的匹配函数：`!match_any_regex(exclude_patterns, val)` 即可（命中则排除）

### Established Patterns
- `CompiledMetaFilters::from_meta(&MetaFilters)` — 扩展此构造函数，从 `meta.exclude_*` 字段编译对应 `Vec<Regex>`，与现有 include 字段并排初始化
- `has_meta_filters: bool` 预计算模式 (`run.rs` FilterProcessor) — 扩展为 `compiled_meta.has_filters_any()` 或等价判断，避免热路径重复检查
- `#[serde(flatten)]` on `MetaFilters` in `FiltersFeature` — 新的 `exclude_*` 字段直接加入 `MetaFilters`，serde flatten 自动处理 TOML 平铺

### Integration Points
- `FiltersFeature::has_filters()` → `self.meta.has_filters()`——扩展 `MetaFilters::has_filters()` 即可传导到 pipeline 激活判断
- `CompiledMetaFilters::should_keep()` — 在现有 include 检查之前插入 exclude OR-veto 短路逻辑
- `FiltersFeature::validate_regexes()` — 末尾追加 7 个 `validate_pattern_list` 调用

</code_context>

<specifics>
## Specific Ideas

- `should_keep()` 重构顺序：先逐字段检查 `exclude_*`（任一命中即 `return false`），再执行现有 include AND 逻辑。与 STATE.md 决策"排除先于包含检查短路更快"一致。
- `CompiledMetaFilters::has_filters()` 可重命名为 `has_include_filters()` 或新增 `has_any_filters()` 方法，以区分"有 include 规则"和"有任意规则（include 或 exclude）"，供 `FilterProcessor::has_meta_filters` 使用。
- init 模板中 verbose 和 minimal 两份配置文本均需更新（`src/cli/init.rs` 约 L84 和 L164 两处）。

</specifics>

<deferred>
## Deferred Ideas

None — 讨论完全在 Phase 8 范围内进行，无超出范围的提案。

</deferred>

---

*Phase: 8-排除过滤器*
*Context gathered: 2026-05-10*

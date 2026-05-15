# Phase 9: CLI 启动提速 - Context

**Gathered:** 2026-05-11
**Status:** Ready for planning

<domain>
## Phase Boundary

消除 CLI 启动路径中的双重 regex 编译（validate 阶段与 compile 阶段各调一次 `Regex::new()`），将 startup update check 无条件移入后台线程，并用 hyperfine 量化冷启动基线。

**不在范围内：** 热路径优化（Phase 10）、新 CLI 命令或子命令、配置格式变更。

</domain>

<decisions>
## Implementation Decisions

### 双重 regex 编译合并（PERF-11 核心）

- **D-01:** `compile_patterns()` 改为接受 `field: &str` 参数并返回 `crate::Result<Option<Vec<Regex>>>`，错误类型为 `ConfigError::InvalidValue`（不再是 `Err(String)`）。这使 compile 本身即作为验证，消除单独的 `validate_pattern_list()` 中的 `Regex::new()` 调用。
- **D-02:** `CompiledMetaFilters::from_meta()` 改为 `try_from_meta() -> crate::Result<CompiledMetaFilters>`。所有 `.expect("regex validated")` 调用改为 `?` 传播错误。
- **D-03:** `FiltersFeature::validate_regexes()` **完全删除**。`Config::validate()` 改为直接调用 `CompiledMetaFilters::try_from_meta()` 并丢弃返回值（只检查错误）。
- **D-04:** validate 命令调用 compile 后丢弃 `CompiledMetaFilters` 结果（不传递给后续流程）。run 命令在构建 `FilterProcessor` 时调用 `try_from_meta()`，使用编译结果。每条代码路径中每个 regex 只编译一次。

### update check 后台化

- **D-05:** `check_for_updates_at_startup()` 改为 `std::thread::spawn` fire-and-forget，无条件后台化，不等待结果。
- **D-06:** 接受后台线程与主流程输出交错（update 警告走 `warn!()` 日志，通过 env_logger 输出到 stderr，不做时序同步）。不需要 `JoinHandle` 或完成信号。

### hyperfine 测量方案

- **D-07:** 测量两个命令：`sqllog2db --version`（纯二进制加载）和 `sqllog2db validate -c config.toml`（含 TOML 解析 + regex 编译），两者差值 ≈ config 加载 + regex 编译耗时。
- **D-08:** 验收报告包含三个对比维度：
  1. 优化前 vs 优化后（Phase 9 实施前后）
  2. 有 regex 配置 vs 无 regex 配置（量化 regex 编译影响）
  3. 优化后的最终数字（最低要求）
- **D-09:** 数据写入 `benches/BENCHMARKS.md`，新增 "Phase 9 CLI 冷启动基线" 节，包含 hyperfine 原始输出。
- **D-10:** 无具体耗时阈值要求——记录数据即验收通过，不设"必须低于 X ms"的硬性门控。只需确认双重编译已消除。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 需求与路线图
- `.planning/REQUIREMENTS.md` — PERF-11 完整描述（双重编译合并、update check 后台化）
- `.planning/ROADMAP.md` §Phase 9 — Success Criteria（4 条验收标准）

### 关键实现文件
- `src/features/filters.rs` — `compile_patterns()`、`validate_pattern_list()`、`CompiledMetaFilters::from_meta()`、`validate_regexes()` 当前实现
- `src/config.rs` — `Config::validate()` 调用链（L54-60）
- `src/cli/update.rs` — `check_for_updates_at_startup()` 当前同步实现
- `src/main.rs` — update check 调用位置（L145）、命令分发结构

### 基准数据
- `benches/BENCHMARKS.md` — 现有 benchmark 记录，Phase 9 数据写入此文件

</canonical_refs>

<code_context>
## Existing Code Insights

### 双重编译当前路径
- **validate 路径**：`Config::validate()` → `filters.validate_regexes()` → `validate_pattern_list(field, ps)` → `Regex::new(p)` ×N
- **compile 路径**：`CompiledMetaFilters::from_meta()` → `compile_patterns(ps)` → `Regex::new(p)` ×N，结果 `.expect("regex validated")`
- 两条路径在每次 `cargo run -- run` 或 `cargo run -- validate` 时都会执行，14 个 regex 字段各编译两遍

### update check 当前结构
- `src/cli/update.rs` `check_for_updates_at_startup()`：同步 GitHub API → `get_latest_release()` → 比较版本号 → `warn!()` 日志
- 调用在 `main.rs:145`，位于命令分发（`match &cli.command`）之前

### Integration Points
- `Config::validate()` 返回 `Result<()>`，改为内部调用 `try_from_meta()` 后签名不变
- `CompiledMetaFilters::try_from_meta()` 替代 `from_meta()`，调用方（`cli/run.rs` `FilterProcessor::new()`）需要处理 `Result`
- `FilterProcessor::new()` 目前不返回 `Result`，可能需要改为 `FilterProcessor::try_new() -> Result<Self>`

</code_context>

<specifics>
## Specific Ideas

- `validate_and_compile()` 这个名字虽然在讨论中提及，但实际实现选择是让 `compile_patterns()` 本身返回 `ConfigError`，而非新增独立函数。planner 按 D-01~D-04 设计，不必引入该函数名。
- 若 `FilterProcessor::new()` 改签名影响较大，planner 可以在 `Config::validate()` 时丢弃 `try_from_meta()` 结果，在 `handle_run()` 中让 `FilterProcessor::try_new()` 返回 `Result<FilterProcessor>`，由 `handle_run()` 用 `?` 处理。

</specifics>

<deferred>
## Deferred Ideas

- 具体的启动时间数字目标（如"必须 <100ms"）——当前不设阈值，数据驱动，未来里程碑可参考 Phase 9 数据设定目标
- `SomeOtherCommand` 后台化——其他命令的类似优化留给后续性能阶段

</deferred>

---

*Phase: 9-CLI 启动提速*
*Context gathered: 2026-05-11*

# Phase 13: TemplateAggregator 流式统计累积器 - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning

<domain>
## Phase Boundary

实现 `TemplateAggregator` 结构体：`observe(key: &str, exectime_us: u64)` 热循环累积 + `finalize() -> Vec<TemplateStats>` 输出统计 + `merge(other: TemplateAggregator)` 支持并行路径合并。通过 `Option<&mut TemplateAggregator>` 侧路径接入 `process_log_file()`，禁用时零开销不变。

本阶段 **不涉及** 结果输出到文件（Phase 14）或图表生成（Phase 15/16）。

</domain>

<decisions>
## Implementation Decisions

### 耗时单位

- **D-01:** hdrhistogram 存储单位为**微秒 (µs)**，转换：`(pm.exectime * 1000.0) as u64`
- **D-02:** `TemplateStats` 耗时字段命名：`avg_us`、`min_us`、`max_us`、`p50_us`、`p95_us`、`p99_us`
- **D-03:** `Histogram::<u64>::new_with_bounds(1, 60_000_000, 2)` — 量程 1µs–60s，sigfig=2（~24 KB/模板，误差 <1%）

### first_seen / last_seen

- **D-04:** `first_seen` / `last_seen` 类型为 `String`，直接 clone `sqllog.ts.as_ref()`，热循环零解析开销
- **D-05:** `merge()` 合并时以字典序比较：`first_seen = min(a.first_seen, b.first_seen)`，`last_seen = max(a.last_seen, b.last_seen)`。达梦日志 ts 为 ISO 8601 格式，字典序与时间顺序一致。

### 聚合开关

- **D-06:** **复用** `template_analysis.enabled` 同时控制归一化（TMPL-01）和聚合（TMPL-02）——`enabled = true` 时两者均激活
- **D-07:** `enabled = true` 是聚合的**前置条件**；若 `enabled = false` 则 `TemplateAggregator` 不创建，`process_log_file()` 收到 `None`，行为与 v1.2 完全一致
- **D-08:** 配置验证层：若用户仅设置 `aggregate = true`（未来扩展）而 `enabled = false`，报错提示"归一化必须先启用"——本阶段不需要此验证，仅单 `enabled` 字段

### 已锁定（来自 ROADMAP/STATE）

- **D-09:** `TemplateAggregator` 不实现 `LogProcessor` trait（`process()` 接收 `&self`，累积需要 `&mut self`；加入 Pipeline 破坏 `pipeline.is_empty()` 快路径）
- **D-10:** 内部使用 `hdrhistogram::Histogram<u64>`（~24 KB/模板），禁止 `Vec<u64>` 全量样本存储
- **D-11:** `observe()` 接收已归一化 key（`normalize_template()` 的输出），不在 `TemplateAggregator` 内部重复归一化
- **D-12:** 并行 CSV 路径：每 rayon task 持有独立 `TemplateAggregator`，主线程通过 `merge()` 合并

### Claude 自行决定

- `TemplateAggregator` 代码放置：新建 `src/features/template_aggregator.rs`（Phase 14 更容易导入）
- 内部 HashMap 类型：`ahash::AHashMap<String, TemplateEntry>`（项目已依赖 `ahash`）
- `TemplateStats` 添加 `#[derive(Debug, Clone, serde::Serialize)]`（Phase 14 需要序列化）
- `hdrhistogram` 版本：与现有 `Cargo.toml` 中已有的版本保持一致，或添加最新稳定版

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 12 输出（直接依赖）
- `src/features/sql_fingerprint.rs` — `normalize_template()` 实现；`observe()` 的 key 来自此函数
- `src/features/mod.rs` — `TemplateAnalysisConfig { enabled: bool }` 结构；`pub use sql_fingerprint::normalize_template` 导出路径；Phase 13 新增 `pub use template_aggregator::TemplateAggregator` 遵循同一模式

### 热循环集成（直接修改）
- `src/cli/run.rs` — `process_log_file()` 函数（当前有 `_do_template: bool` 占位参数，Phase 13 替换为 `aggregator: Option<&mut TemplateAggregator>`）；`process_csv_parallel()` 函数（每 rayon task 创建独立 aggregator，返回后主线程 merge）；`handle_run()` 函数（创建 `TemplateAggregator`，传入，最后 `finalize()`）

### 并行路径参考
- `src/cli/run.rs` — 现有 rayon parallel CSV 流程（`process_csv_parallel` 函数），map-reduce 模式参照对象

### 需求定义
- `.planning/ROADMAP.md` §"Phase 13" — 5 条成功标准（完整接入规范）
- `.planning/REQUIREMENTS.md` §"TMPL-02" — 功能需求原文
- `.planning/STATE.md` §"Decisions (v1.3)" — 锁定决策表（D-09 ~ D-12 来源）

### 配置参考
- `src/config.rs` — `FeaturesConfig`；`apply_one()` 中 `features.replace_parameters.enable` 的处理模式是 `features.template_analysis.enabled` 的参照对象

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ahash::AHashMap` — 项目已依赖，用于 `TemplateAggregator` 内部 `HashMap<String, TemplateEntry>`
- `f32_ms_to_i64()` in `src/exporter/mod.rs` — 理解 `pm.exectime` 的 f32 ms 单位；Phase 13 用 `(pm.exectime * 1000.0) as u64` 转 µs，不复用此函数（语义不同）
- `pub use sql_fingerprint::fingerprint` 模式 in `src/features/mod.rs` — 新增 `pub use template_aggregator::TemplateAggregator` 遵循同一导出模式

### Established Patterns
- **侧路径参数模式**：`process_log_file()` 已有 `_do_template: bool`（Phase 12 占位）；Phase 13 将其替换为 `aggregator: Option<&mut TemplateAggregator>`，同时删除 `_do_template`（用 `aggregator.is_some()` 判断）
- **条件调用守卫**：`if do_normalize && field_active { compute_normalized(...) }` — `aggregator` 的调用遵循相同的 `if let Some(agg) = aggregator { agg.observe(key, exectime_us) }` 模式
- **finalize 生命周期**：参考 `exporter_manager.finalize()` 在 `handle_run()` 最后调用；`aggregator.finalize()` 同位置调用

### Integration Points
- `process_log_file()` 参数列表（`src/cli/run.rs:114`）→ 新增 `aggregator: Option<&mut TemplateAggregator>` 参数
- `process_csv_parallel()` 内 rayon 任务闭包 → 每任务 `let mut agg = TemplateAggregator::new(); ... agg`，collect 后主线程 reduce
- `src/features/mod.rs` → 新增 `pub mod template_aggregator; pub use template_aggregator::TemplateAggregator;`

</code_context>

<specifics>
## Specific Ideas

- `observe()` 热路径应该尽量轻量：key 用 `&str` 而非 `String`（避免每次分配），内部用 `entry().or_insert_with(TemplateEntry::new)` 访问；`TemplateEntry` 持有 `Histogram<u64>` + count + first_seen + last_seen
- `finalize()` 返回 `Vec<TemplateStats>`，按 `count desc` 排序（最频繁优先，DBA 最关心）
- `merge()` 需要 hdrhistogram 的 `add()` 方法合并直方图；注意 `add()` 要求两个 histogram 的量程相同（均为 `new_with_bounds(1, 60_000_000, 2)`，天然满足）

</specifics>

<deferred>
## Deferred Ideas

- `TemplateAnalysisConfig` 后续字段（`top_n: usize` 等）— 推迟到 Phase 14/15 按需添加（来自 Phase 12 决策，继续推迟）
- 独立 JSON/CSV 报告输出（TMPL-03/TMPL-03b）— Future Requirements v1.4+
- 单独的 `aggregate: bool` 字段（如未来 TMPL-01 和 TMPL-02 需要独立控制）— 现阶段复用 `enabled` 足够，未来按需添加

### Reviewed Todos (not folded)
- "调研 dm-database-parser-sqllog 1.0.0 新特性"（todo score 0.4）— 关键词匹配，但与 Phase 13 实现无直接关联，不折叠

</deferred>

---

*Phase: 13-TemplateAggregator 流式统计累积器*
*Context gathered: 2026-05-16*

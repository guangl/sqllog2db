# Phase 16: 剩余图表 - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning

<domain>
## Phase Boundary

在 Phase 15 的 `src/charts/` 基础设施和 `ChartsConfig` 上，新增两类图表：
- `frequency_trend.svg`：时间趋势折线图（X 轴小时粒度，Y 轴全局 SQL 执行总次数）
- `user_schema_pie.svg`：按用户执行占比饼图

本阶段**不涉及** TMPL-03 独立报告输出（Future v1.4+）。

</domain>

<decisions>
## Implementation Decisions

### 时间趋势数据收集（CHART-04）

- **D-01:** 在 `TemplateAggregator` 内部新增 `hour_counts: BTreeMap<String, u64>`，`observe()` 在记录每条 SQL 时顺带 bucket 计数。无需独立 struct，避免 run.rs 维护多个 aggregator。
- **D-02:** Hour bucket key = `ts[..13]`（取前 13 字符，如 `"2025-01-15 10"`）。达梦时间戳格式 `"YYYY-MM-DD HH:MM:SS"` 字典序与时间序一致，直接切片即可，无需 chrono 解析。
- **D-03:** 暴露接口 `iter_hour_counts()`，返回按 bucket key 升序排列的 `Iterator<Item=(&str, u64)>`，与 `iter_chart_entries()` 风格一致。
- **D-04:** `TemplateAggregator::merge()` 需相应合并 `hour_counts`（`for (k, v) in other.hour_counts { *self.hour_counts.entry(k).or_insert(0) += v; }`）。

### 饼图数据字段（CHART-05）

- **D-05:** `observe()` 新增 `user: &str` 参数（完整签名：`observe(&mut self, key: &str, exectime_us: u64, ts: &str, user: &str)`）。内部维护 `user_counts: AHashMap<String, u64>`，与 `hour_counts` 同在一次 observe 调用中更新。
- **D-06:** 饼图按 `username` 分组，图表标题写 "SQL Executions by User"（不用 Schema，因 MetaParts 无独立 schema 字段；DaMeng 用户即 schema 所有者语义等价）。
- **D-07:** 输出文件名保持 `user_schema_pie.svg`（符合 ROADMAP 命名），标题文字改为 "By User"。
- **D-08:** 饼图用现有 `ChartsConfig.top_n` 限制最多显示 N 个用户扇区，其余合并为 "Others" 扇区（当用户总数 > top_n 时）。
- **D-09:** `iter_user_counts()` 接口，返回按 count 降序排列的 `Iterator<Item=(&str, u64)>`，供 `user_pie.rs` 使用。

### ChartsConfig 新开关字段

- **D-10:** `ChartsConfig` 新增两个 bool 字段，字段名与现有 `frequency_bar`/`latency_hist` 风格一致：
  ```toml
  [features.charts]
  output_dir = "charts/"
  top_n = 10
  frequency_bar = true
  latency_hist = true
  trend_line = true    # Phase 16 新增，默认 true
  user_pie = true      # Phase 16 新增，默认 true
  ```
- **D-11:** `generate_charts()` 内部按 `cfg.trend_line`/`cfg.user_pie` 开关分发，分别调用 `src/charts/trend_line.rs` 和 `src/charts/user_pie.rs`（Phase 15 CONTEXT.md D-11 已规划的子模块）。

### Claude 的自行决定

- 折线图 SVG 尺寸（与 Phase 15 条形图保持 1200×600 或单独调整）
- X 轴时间标签格式（如显示 "10:00"、"2025-01-15 10" 还是截断）
- 饼图颜色主题（区分 top N 用户的多色方案，plotters 内置调色板）
- "Others" 扇区颜色（建议灰色）

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 15 输出（直接依赖）
- `src/features/template_aggregator.rs` — `TemplateAggregator`（当前 `ChartEntry`/`iter_chart_entries()` 结构，Phase 16 在此基础上新增 `hour_counts`/`user_counts` + 两个 iter 方法 + 修改 `observe()` 签名）
- `src/features/mod.rs` — `ChartsConfig` struct（当前字段，Phase 16 在此新增 `trend_line`/`user_pie`）
- `.planning/phases/15-svg/15-CONTEXT.md` — D-01~D-13（特别 D-11 模块结构规划、D-04/D-05 ChartsConfig 字段定义）

### 需求与架构参考
- `.planning/ROADMAP.md` §"Phase 16" — 成功标准（4 条 SC，含 X 轴小时标签可读性和长名称截断要求）
- `.planning/REQUIREMENTS.md` §"CHART-04, CHART-05" — 功能需求原文
- `.planning/STATE.md` §"Decisions (v1.3)" — 锁定决策（plotters SVG-only，禁止 charts-rs/charming）

### 调用集成点
- `src/cli/run.rs` — `handle_run()` 中 `generate_charts()` 调用点（Phase 15 Plan 05 接入后的代码）；`agg.observe()` 调用点（需更新为新签名 + 传 `meta.username`）

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `TemplateAggregator::iter_chart_entries()` — 返回 `ChartEntry { key, count, histogram }`，Phase 16 的 `iter_hour_counts()` / `iter_user_counts()` 遵循同一模式
- `ensure_parent_dir()` in `src/exporter/mod.rs` — 确保输出目录存在，Phase 15 Plan 05 接入后图表生成已复用
- `ahash::AHashMap` — 项目已有依赖，用于 `user_counts`
- `std::collections::BTreeMap` — 标准库，用于 `hour_counts`（排序保证时间轴有序）

### Established Patterns
- `observe()` 热路径模式：当前签名 `observe(&mut self, key: &str, exectime_us: u64, ts: &str)`，Phase 16 追加 `user: &str`；调用方（run.rs）传 `meta.username.as_ref()`
- `merge()` 合并模式：当前合并 `entries + first/last_seen`，Phase 16 同样合并 `hour_counts` + `user_counts`（rayon 并行路径自动支持）
- 函数长度 ≤ 40 行（CLAUDE.md 约束，图表渲染逻辑需拆分为小函数）

### Integration Points
- `src/charts/mod.rs` — `generate_charts()` 入口，Phase 16 新增 `trend_line.rs`/`user_pie.rs` 并在入口按开关分发
- `src/main.rs` — `mod charts;` 已在 Phase 15 Plan 03/05 接入（Phase 15 完成后确认）

</code_context>

<specifics>
## Specific Ideas

- Hour bucket key 格式示例：`"2025-01-15 10"` → X 轴标签可显示为 `"01-15 10:00"` 或 `"10:00"`（当 span 跨越多天时需显示日期）
- 饼图 "Others" 条件：`user_counts.len() > top_n` 时，按 count 降序取前 top_n，其余求和归入 "Others"
- `observe()` 签名变更影响：run.rs 的单线程路径（`agg.observe(...)`）和并行路径（`task_agg.observe(...)`）均需更新，传入 `meta.username.as_ref()`

</specifics>

<deferred>
## Deferred Ideas

- 按 `statement` 字段（SELECT/INSERT/UPDATE/DELETE 等）生成操作类型饼图 — 超出 Phase 16 scope，可作 Future v1.4+ 考虑
- 可配置图表宽高（`[features.charts] width/height`）— 当前固定尺寸已足够

</deferred>

---

*Phase: 16-剩余图表*
*Context gathered: 2026-05-16*

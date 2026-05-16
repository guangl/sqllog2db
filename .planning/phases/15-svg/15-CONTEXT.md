# Phase 15: SVG 图表基础设施 + 前两类图表 - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning

<domain>
## Phase Boundary

实现 `ChartsConfig` 配置段 + `src/charts/` 新模块 + plotters SVG-only 后端。在 `handle_run` 中，于 `TemplateAggregator::finalize()` **之前** 调用 `generate_charts(agg, cfg)`，生成两类 SVG 文件：`top_n_frequency.svg`（Top N 横向条形图）和 `latency_histogram_<key>.svg`（Top N 模板各自的耗时分布直方图）。

本阶段 **不涉及** 时间趋势折线图和用户/Schema 饼图（Phase 16），也不涉及 TMPL-03 独立 JSON/CSV 报告（Future v1.4+）。

</domain>

<decisions>
## Implementation Decisions

### 直方图桶数据来源

- **D-01:** 图表生成在 `TemplateAggregator::finalize()` **之前**发生，直接访问 `TemplateAggregator` 内部原始 `Histogram<u64>`，调用 `iter_recorded()` 获取 bucket 数据。调用顺序：`generate_charts(agg, cfg)?` → `finalize()` → `write_template_stats()`
- **D-02:** 每个 Top N 模板各自生成一张独立的耗时分布直方图，文件名为 `latency_histogram_<sanitized_key>.svg`（key 经过文件名安全处理：非 ASCII 字母数字替换为 `_`，截断到合理长度）
- **D-03:** 耗时直方图 X 轴使用**对数刻度**（如 1µs, 10µs, 100µs, 1ms, 10ms...），避免小耐时 bucket 被大耐时 bucket 挤压不可见。plotters 的 `LogScaleRangeValue` 或手动对数坐标系处理。

### ChartsConfig 结构

- **D-04:** 独立 `[features.charts]` 配置段，与 `[features.template_analysis]` 解耦。`ChartsConfig` 新增至 `FeaturesConfig` 作为 `charts: Option<ChartsConfig>`
- **D-05:** `ChartsConfig` 字段：
  ```toml
  [features.charts]
  output_dir = "charts/"
  top_n = 10              # 默认 10，控制两类图表的模板数量
  frequency_bar = true    # 默认 true
  latency_hist = true     # 默认 true
  ```
- **D-06:** `Config::validate()` 中：若 `features.charts` 存在，则 `features.template_analysis.enabled` 必须为 `true`；否则报错提示 "启用 [features.charts] 需要先设置 [features.template_analysis]\nenabled = true"

### Top N 频率条形图

- **D-07:** `top_n_frequency.svg` 为**横向**条形图，Y 轴为模板 key，X 轴为执行次数（count），按频率降序排列（最高频率在最上方）
- **D-08:** Y 轴标签截断策略：超过 40 字符时截断并追加 `"…"`
- **D-09:** 图表尺寸：**1200×600 像素**；边距、标题、字体大小由 Claude 根据 plotters 最佳实践决定
- **D-10:** X 轴单位为执行次数（count），不转换为百分比

### 模块代码结构

- **D-11:** 图表代码放在新模块 `src/charts/`，子模块划分：
  - `src/charts/mod.rs` — 公共入口：`pub fn generate_charts(agg: &TemplateAggregator, cfg: &ChartsConfig) -> Result<()>`
  - `src/charts/frequency_bar.rs` — Top N 频率条形图实现
  - `src/charts/latency_hist.rs` — 耗时分布直方图实现（Phase 16 新增 `trend_line.rs`、`user_pie.rs`）
- **D-12:** `run.rs` 单一调用点：`generate_charts(&agg, charts_cfg)?`，内部按 `cfg.frequency_bar`/`cfg.latency_hist` 开关分发
- **D-13:** `src/main.rs` 新增 `mod charts;`

### Claude 的自行决定

- plotters 版本：选最新稳定版（写入 Cargo.toml 时检查 crates.io）
- plotters feature flags：使用 `svg` backend only，禁用 `bitmap_backend`，避免字体/图像系统依赖
- 直方图每模板文件名的 sanitize 逻辑细节（非 ASCII/非数字字符替换为 `_`，文件名长度上限）
- 条形图颜色主题（单色 steelblue 系，无需渐变，与 CLI 工具风格一致）
- 图表标题格式（如 `Top 10 SQL Templates by Frequency`，包含运行时 N 值）
- `latency_hist.svg` 的图表尺寸（与条形图相同使用 1200×600，或单独调整）

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 13 输出（直接依赖）
- `src/features/template_aggregator.rs` — `TemplateAggregator` struct（`observe`、`finalize`、`merge`）；`TemplateEntry` 内部持有 `Histogram<u64>`（Phase 15 在 finalize 前直接访问 histogram）；`TemplateStats` struct（p50/p95/p99 字段，供条形图工具提示参考）
- `.planning/phases/13-templateaggregator/13-CONTEXT.md` — D-01~D-12（耗时单位 µs、hdrhistogram 配置 `new_with_bounds(1, 60_000_000, 2)`）

### 现有配置结构（直接修改）
- `src/features/mod.rs` — `FeaturesConfig`（新增 `charts: Option<ChartsConfig>`）；`TemplateAnalysisConfig`（D-06 验证依赖此字段）
- `src/config.rs` — `Config::validate()` / `validate_and_compile()`（新增 charts 依赖验证）；`apply_one()` 如有 charts 字段需支持 `--set` 覆盖

### 需求与架构参考
- `.planning/ROADMAP.md` §"Phase 15" — 5 条成功标准（SC-1~SC-5）
- `.planning/ROADMAP.md` §"Phase 16" — 后续图表规划（Phase 15 的基础设施须为其留好扩展点）
- `.planning/REQUIREMENTS.md` §"CHART-01, CHART-02, CHART-03" — 功能需求原文
- `.planning/STATE.md` §"Decisions (v1.3)" — 锁定决策：plotters SVG-only，禁止 charts-rs 和 charming

### 调用集成点
- `src/cli/run.rs` — `handle_run()` 函数（图表生成插入点：在 `exporter_manager.finalize()?` **之前**，`template_agg` 仍可变引用时）；顺序路径约 L905，并行路径约 L804 附近

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `hdrhistogram::Histogram<u64>`：`TemplateEntry.histogram` 字段（private）；Phase 15 需要通过 `TemplateAggregator` 暴露访问接口，或在 finalize 前传入 chart generator。研究阶段需确认 `TemplateEntry` 是否需要添加 pub 访问方法或 `TemplateAggregator` 是否提供 chart-oriented 迭代接口
- `itoa`：已在 CSV exporter 使用，Phase 15 SVG 文本中的整数格式化可复用
- `ensure_parent_dir()` in `src/exporter/mod.rs`：写 SVG 文件前确保 output_dir 存在，可直接复用此辅助函数（或内联等价逻辑）

### Established Patterns
- `Option<ChartsConfig>` 模式：与 `features.filters: Option<FiltersFeature>`、`features.replace_parameters: Option<ReplaceParametersConfig>` 完全一致
- 配置验证层：`Config::validate()` 中已有跨字段依赖检查（如 fields 字段名验证），Phase 15 的 charts→template_analysis 依赖验证遵循同一模式
- `flush()?` 显式调用：SC-4 要求每个 SVG write 函数关闭前显式 `flush()`，与 `CsvExporter` 的 `writer.flush()?` 保持一致
- 函数长度 ≤ 40 行（CLAUDE.md 约束）：图表子函数须拆分

### Integration Points
- `handle_run()` 调用顺序修改：
  ```rust
  // 当前（Phase 14 之后）：
  exporter_manager.finalize()?;
  let template_stats = template_agg.map(TemplateAggregator::finalize);
  if let Some(ref stats) = template_stats {
      exporter_manager.write_template_stats(stats, None)?;
  }

  // Phase 15 新顺序：
  if let Some(ref agg) = template_agg {
      if let Some(charts_cfg) = config.features.charts.as_ref() {
          charts::generate_charts(agg, charts_cfg)?;
      }
  }
  exporter_manager.finalize()?;
  let template_stats = template_agg.map(TemplateAggregator::finalize);
  if let Some(ref stats) = template_stats {
      exporter_manager.write_template_stats(stats, None)?;
  }
  ```
- 并行路径：`parallel_agg` 在 `process_csv_parallel()` 返回后，同样需要在 `finalize()` 前调用图表生成

</code_context>

<specifics>
## Specific Ideas

- 文件名 sanitize：`latency_histogram_{}.svg`，key 中非 `[a-zA-Z0-9_-]` 字符替换为 `_`，整体截断到 80 字符（避免文件系统路径过长）
- `TemplateAggregator` 可能需要新增迭代接口（如 `iter_entries()`），供图表生成函数访问每个模板的 histogram；这应作为 `pub` 方法暴露在 `template_aggregator.rs`，研究阶段确认最佳接口形式
- `generate_charts()` 在 `output_dir` 不存在时自动创建（SC-5：未启用时不创建目录；启用时创建）

</specifics>

<deferred>
## Deferred Ideas

- 时间趋势折线图（`frequency_trend.svg`）— Phase 16
- 用户/Schema 占比饼图（`user_schema_pie.svg`）— Phase 16
- 可配置图表宽高（`[features.charts] width/height`）— 当前固定 1200×600 已足够
- 独立 JSON/CSV 统计报告（TMPL-03/03b）— Future v1.4+

</deferred>

---

*Phase: 15-SVG 图表基础设施 + 前两类图表*
*Context gathered: 2026-05-16*

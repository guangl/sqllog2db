# Phase 15: SVG 图表基础设施 + 前两类图表 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 15-SVG 图表基础设施 + 前两类图表
**Areas discussed:** 直方图桶数据来源, ChartsConfig 结构设计, Top N 条形图细节, 模块代码结构

---

## 直方图桶数据来源

| Option | Description | Selected |
|--------|-------------|----------|
| 在 finalize() 之前生成图表 | TemplateAggregator 在 finalize() 之前直接传入图表生成函数，访问原始 histogram。调用顺序：图表生成 → finalize() → write_template_stats | ✓ |
| 将桶数据序列化进 TemplateStats | 在 TemplateStats 中新增 Vec<(u64, u64)> 存入 iter_recorded() 桶，finalize 时提取 | |
| 仅用 p50/p95/p99 建构近似直方图 | 不依赖原始 histogram，用三个百分位合成展示用直方图。精度较低且不符合 ROADMAP SC-3 | |

**User's choice:** 在 finalize() 之前生成图表（推荐）

| Follow-up | Option | Selected |
|-----------|--------|----------|
| 直方图粒度 | 全局合并：所有模板耗时合并到一张图 | |
| 直方图粒度 | 每模板独立：Top N 模板各一张图 | ✓ |
| X 轴刻度 | 对数轴 | ✓ |
| X 轴刻度 | 线性轴 | |

**Notes:** 对数轴符合 hdrhistogram bucket 的天然对数分布特性，避免小耐时区间不可见。

---

## ChartsConfig 结构设计

| Option | Description | Selected |
|--------|-------------|----------|
| 独立 [features.charts] 段 | 与 template_analysis 完全解耦，validate() 检查依赖关系 | ✓ |
| 嵌套在 template_analysis 下 | [features.template_analysis.charts] 嵌套语义强制依赖，但 TOML 语法较麻烦 | |

| Follow-up | Option | Selected |
|-----------|--------|----------|
| 字段设计 | output_dir + top_n（最小化） | |
| 字段设计 | output_dir + top_n + 每图类独立开关（frequency_bar/latency_hist） | ✓ |
| 缺失依赖时 | 报错 | ✓ |
| 缺失依赖时 | 静默忽略 | |

**User's choice:** 独立段 + output_dir + top_n + 每图类开关 + 缺失依赖时报错

---

## Top N 条形图细节

| Option | Description | Selected |
|--------|-------------|----------|
| Y 轴标签截断 60 字符 + … | 较宽松，多数 SQL 类型可识别 | |
| Y 轴标签截断 40 字符 + … | 更紧凑，适合模板数量多时 | ✓ |
| Claude 自行决定 | 根据 plotters 画布尺寸动态调整 | |

| Follow-up | Option | Selected |
|-----------|--------|----------|
| 图表尺寸 | 1200×600 像素，Claude 决定边距和标题 | ✓ |
| 图表尺寸 | 800×500 像素 | |
| 图表尺寸 | 可配置宽高 | |
| X 轴指标 | 执行次数（count） | ✓ |
| X 轴指标 | 百分比（%） | |

**Notes:** 横向条形图，频率降序（最高频率在最上方）。

---

## 模块代码结构

| Option | Description | Selected |
|--------|-------------|----------|
| src/charts/mod.rs（新模块） | 独立模块，与 exporter/ 对称；Phase 16 扩展 trend_line.rs/user_pie.rs | ✓ |
| src/cli/charts.rs（嵌入 CLI 层） | 贴近调用方，但混入 CLI 和渲染逻辑 | |
| src/exporter/charts.rs（嵌入 exporter 层） | 图表与主记录流导出差异较大，架构不合 | |

| Follow-up | Option | Selected |
|-----------|--------|----------|
| 公共接口 | 单一入口 generate_charts(agg, cfg) | ✓ |
| 公共接口 | 多函数分别调用 | |

**Notes:** src/charts/ 子模块：frequency_bar.rs + latency_hist.rs；Phase 16 扩展 trend_line.rs + user_pie.rs。

---

## Claude's Discretion

- plotters 版本：最新稳定版（写入 Cargo.toml 时检查 crates.io）
- plotters feature flags：svg only，禁用 bitmap_backend
- 直方图文件名 sanitize 逻辑细节（非 ASCII/非数字字符替换为 `_`，上限长度）
- 条形图颜色主题（单色 steelblue 系）
- 图表标题格式（如 `Top 10 SQL Templates by Frequency`）
- 耗时直方图画布尺寸（与条形图相同或单独调整）

## Deferred Ideas

- 时间趋势折线图（`frequency_trend.svg`）— Phase 16
- 用户/Schema 占比饼图（`user_schema_pie.svg`）— Phase 16
- 可配置图表宽高（`width/height` 字段）— 当前固定 1200×600 已足够，未来需要再加
- 独立 JSON/CSV 统计报告（TMPL-03/03b）— Future v1.4+

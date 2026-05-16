# Phase 16: 剩余图表 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 16-剩余图表
**Areas discussed:** 时间趋势数据收集, 饼图数据字段, ChartsConfig 新开关字段名

---

## 时间趋势数据收集

| Option | Description | Selected |
|--------|-------------|----------|
| 扩展 TemplateAggregator | observe() 已有 ts，内部新增 BTreeMap bucket；merge() 自动支持并行路径 | ✓ |
| 独立 TrendCollector struct | charts 模块新 struct，run.rs 维护两个 aggregator | |
| run.rs 内联局部变量 | 最简，并行路径 merge 需额外处理 | |

**User's choice:** 扩展 TemplateAggregator

---

| Option | Description | Selected |
|--------|-------------|----------|
| 取前 13 字符 | "2025-01-15 10" — 字典序即时间序，直接切片 | ✓ |
| chrono 解析小时 | 更严谨但多一个函数调用 | |

**User's choice:** 取前 13 字符

---

| Option | Description | Selected |
|--------|-------------|----------|
| iter_hour_counts() | 返回 sorted Iterator，与 iter_chart_entries() 风格一致 | ✓ |
| hour_counts() | 返回 &BTreeMap 引用，调用方自行迭代 | |

**User's choice:** iter_hour_counts()

---

## 饼图数据字段

| Option | Description | Selected |
|--------|-------------|----------|
| username 字段，标题 "By User/Schema" | MetaParts.username 即 DaMeng schema 所有者 | |
| username 字段，标题只写 "By User" | 更明确直接，不混淆 schema 概念 | ✓ |
| statement 字段（按操作类型） | 按 SELECT/INSERT 等分组，不是原始需求 | |

**User's choice:** username 字段，标题只写 "By User"

---

| Option | Description | Selected |
|--------|-------------|----------|
| 同样扩展 TemplateAggregator | observe() 新增 user 参数，与 hour_counts 共存 | ✓ |
| 独立收集（run.rs 局部变量） | 分离关注点，并行路径需额外 merge | |

**User's choice:** 同样扩展 TemplateAggregator

---

| Option | Description | Selected |
|--------|-------------|----------|
| 不限制，显示所有用户 | 生产环境用户数通常 < 20 | |
| 用现有 top_n 控制 | 前 top_n 个 + "Others"，防止扇区过多 | ✓ |

**User's choice:** 用现有 top_n 控制

---

## ChartsConfig 新开关字段名

| Option | Description | Selected |
|--------|-------------|----------|
| trend_line / user_pie | 与 frequency_bar / latency_hist 风格一致（图表类型简称） | ✓ |
| frequency_trend / user_schema_pie | 与输出文件名完全对应，但更长 | |

**User's choice:** trend_line / user_pie

---

## Claude's Discretion

- 折线图 SVG 尺寸（参考 Phase 15 条形图 1200×600）
- X 轴时间标签格式（单天 vs 跨天场景）
- 饼图多色调色板和 "Others" 扇区颜色（建议灰色）

## Deferred Ideas

- 按 statement 字段（操作类型）生成饼图 — Future v1.4+
- 可配置图表宽高 — Future

# Phase 13: TemplateAggregator 流式统计累积器 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 13-TemplateAggregator
**Areas discussed:** 耗时单位, first_seen/last_seen 类型, 聚合开关设计

---

## 耗时单位

| Option | Description | Selected |
|--------|-------------|----------|
| 微秒 µs | `(pm.exectime * 1000.0) as u64`；字段名 avg_us/p95_us；精度更高 | ✓ |
| 毫秒 ms | `pm.exectime as u64`；字段名 avg_ms；与现有 exec_time_ms 列一致 | |

**User's choice:** 微秒 µs

| Option | Description | Selected |
|--------|-------------|----------|
| sigfig=2，~24 KB/模板 | 误差 <1%，与 REQUIREMENTS.md TMPL-02 约束对齐 | ✓ |
| sigfig=3，~64 KB/模板 | 误差 <0.1%，内存 2.7x，对 DBA 场景精度过剩 | |

**User's choice:** sigfig=2

| Option | Description | Selected |
|--------|-------------|----------|
| high=60_000_000 µs（60秒） | 覆盖绝大多数查询，超出截断而非崩溃 | ✓ |
| high=3_600_000_000 µs（1小时） | 完全不截断，但 bucket 数量更多，内存略大 | |

**User's choice:** 60_000_000 µs

---

## first_seen/last_seen 类型

| Option | Description | Selected |
|--------|-------------|----------|
| String 原始字符串 | 直接 clone sqllog.ts，零解析开销；Phase 14 SQLite 存 TEXT | ✓ |
| i64 Unix 时间戳 ms | 每条记录额外 chrono 解析；SQLite 存 INTEGER，支持范围查询 | |

**User's choice:** String 原始字符串

| Option | Description | Selected |
|--------|-------------|----------|
| 字典序比较 min/max | first_seen=min, last_seen=max；达梦 ts 为 ISO 8601，字典序与时序一致 | ✓ |
| 只保留其中一个的值 | 合并结果不确定 | |

**User's choice:** 字典序比较 min/max

---

## 聚合开关设计

| Option | Description | Selected |
|--------|-------------|----------|
| 复用 enabled 同时控制两者 | `enabled=true` 同时激活归一化（TMPL-01）和聚合（TMPL-02） | ✓ |
| 新增 aggregate: bool 字段 | 两个独立开关，用户可单独启用归一化而不承担聚合开销 | |

**User's choice:** 复用 enabled

| Option | Description | Selected |
|--------|-------------|----------|
| enabled=true 是前置条件 | 逻辑连贯：无归一化 key 则无法聚合；配置验证层报错提示 | ✓ |
| 两者完全独立 | 允许用原始 SQL 为 key 聚合，但模板数量会爆炸 | |

**User's choice:** enabled=true 是前置条件

---

## Claude's Discretion

- `TemplateAggregator` 代码放置于新建 `src/features/template_aggregator.rs`
- 内部 HashMap 类型：`ahash::AHashMap<String, TemplateEntry>`
- `TemplateStats` 添加 `#[derive(Debug, Clone, serde::Serialize)]`
- `finalize()` 返回 `Vec<TemplateStats>` 按 count desc 排序

## Deferred Ideas

- 独立 `aggregate: bool` 字段（如未来需要 TMPL-01/TMPL-02 独立控制）
- `TemplateAnalysisConfig` 后续字段（top_n 等）推迟到 Phase 14/15
- TMPL-03/TMPL-03b 独立报告输出延至 v1.4+

# Phase 4: CSV 性能优化 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-27
**Phase:** 04-csv
**Areas discussed:** 量化策略, BufWriter 调参, 10% 目标兜底方案, 验收方式

---

## 量化策略

| Option | Description | Selected |
|--------|-------------|----------|
| 先量化，再优化 | Wave 0 加 bench_csv_format_only，确认格式化层净开销后再决定是否值得改动 | ✓ |
| 直接优化，边跑边量 | 跳过格式化隔离，直接对 reserve 策略和调用时序做最有把握的改动 | |

**User's choice:** 先量化，再优化

---

| Option | Description | Selected |
|--------|-------------|----------|
| 接受 pub(crate) | 仅库内可见，不成为公开 API | ✓ |
| 保持私有，通过公开路径测 | 通过 export_one_preparsed 间接测，量化结果不够纯粹 | |

**User's choice:** 接受 pub(crate)

---

| Option | Description | Selected |
|--------|-------------|----------|
| 硬编码典型记录 | 明确、可重复，不依赖真实日志文件即可运行 | ✓ |
| 复用现有 fixture | 减少重复代码，但现有 bench 可能无预解析记录 | |

**User's choice:** 硬编码典型记录，吸删量 10000 条

---

## BufWriter 调参

| Option | Description | Selected |
|--------|-------------|----------|
| 保持 16MB，不调参 | 研究已确认差异极小，OS page cache 负责缓冲 | ✓ |
| 实验 64KB / 1MB / 4MB | 有实测数据作支撑，但额外工作量 | |

**User's choice:** 保持 16MB 不变

---

## 10% 目标兜底方案

| Option | Description | Selected |
|--------|-------------|----------|
| 允许 lazy parse_pm | 若格式化层不足，拓展到调用层——配置项控制 parse_pm 是否调用 | ✓ |
| 仅限格式化层 | 严格局限于 csv.rs，不足则降低验收标准或留 Phase 5+ | |
| 完全不限制范围 | 只要不修改上游 crate，其余均可 | |

**User's choice:** 允许 lazy parse_pm（配置项控制）

---

| Option | Description | Selected |
|--------|-------------|----------|
| 配置项控制（推荐） | `export.include_performance_metrics = true/false`，默认 true | ✓ |
| 按记录类型跳过 | 上游 crate API 不确定是否支持 | |
| 先尝试再决定 | Wave 0 量化后再定具体方案 | |

**User's choice:** 配置项控制，默认 true

---

## 验收方式

| Option | Description | Selected |
|--------|-------------|----------|
| criterion real-file 数字（主） | csv_export_real/real_file --baseline v1.0，median 降低 ≥10% | ✓ |
| synthetic benchmark 主 + real-file 展示 | csv_export/10000 为主验收，real-file 只展示不要求达标 | |

**User's choice:** criterion real-file 数字为主验收

---

| Option | Description | Selected |
|--------|-------------|----------|
| 可选（强烈建议但不强制） | criterion 数字是确凿主验收；flamegraph 如时间允许则生成 | ✓ |
| 必须做 | PERF-08 "显著减少堆分配"需要 flamegraph 对比可见差异 | |

**User's choice:** flamegraph diff 可选，强烈建议但不强制

---

## Claude's Discretion

- bench_csv_format_only 的具体字段值（ts, ep, trxid, sql 内容）由 Claude 决定，用户选择"硬编码典型记录"
- lazy parse_pm 配置项接入位置（CsvExporter 内还是 cli/run.rs 热循环）待 Wave 0 量化后由 Claude 决定

## Deferred Ideas

- BufWriter 容量实验（64KB/1MB/4MB）— 研究假设差异极小，暂不做
- find_indicators_split() 调用次数减少 — 上游 crate 不可修改，Phase 6 评估

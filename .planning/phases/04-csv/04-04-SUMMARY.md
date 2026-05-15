---
phase: 04-csv
plan: 04
subsystem: benchmarking-verification
tags: [benchmark, verification, phase-4, perf-02, perf-03, perf-08, checkpoint]

# Dependency graph
requires:
  - phase: 04-csv
    plan: 03
    provides: Wave 2 include_performance_metrics 配置项 + 热循环集成

provides:
  - Phase 4 最终 benchmark 结果（benches/BENCHMARKS.md Phase 4 段落）
  - Phase 4 验收报告（.planning/phases/04-csv/04-VERIFICATION.md）
  - PERF-02/03/08 三项需求的实测证据（含 fail 原因分析）

affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "criterion --baseline v1.0 对比 Phase 4 与 v1.0 的 CSV synthetic benchmark 提升"
    - "target/phase4-bench/*.txt 保存 criterion 原始输出作为审计记录"

key-files:
  created:
    - .planning/phases/04-csv/04-VERIFICATION.md
  modified:
    - benches/BENCHMARKS.md

key-decisions:
  - "PERF-02 fail 原因：合成 benchmark -8.53% 接近但未达 10%；csv_export_real 因 sqllogs/ 不存在无法采集；主要热路径（parse_meta, LogIterator::next）在上游解析 crate 不可控"
  - "推荐 accept-defer：Phase 4 已穷尽本 phase 内可控优化，上游解析热路径留 Phase 6 评估新 API"
  - "PERF-03/08 已达成：格式化层条件 reserve（Plan 02）+ include_pm=false 兜底方案（Plan 03）均已实施"

requirements-completed: []

# Metrics
duration: ~30min
completed: 2026-05-09
---

# Phase 04 Plan 04: Phase 4 收尾 Benchmark 与验收 Summary

**Phase 4 最终 benchmark 对比：csv_export/10000 vs v1.0 = -8.53%（合成 benchmark），未达 10% 目标；csv_export_real 无法采集（sqllogs/ 缺失）；BENCHMARKS.md 和 04-VERIFICATION.md 已记录所有实测数值与分析，等待 human-verify 决议**

## Performance

- **Duration:** ~30 min
- **Completed:** 2026-05-09
- **Tasks:** 3/3（Task 3 checkpoint:human-verify 已完成，accept-defer 决议已记录）
- **Files modified:** 2（benches/BENCHMARKS.md 新增 Phase 4 段落，.planning/phases/04-csv/04-VERIFICATION.md 新建）

## Benchmark 最终数值（Phase 4 vs v1.0）

测试环境：Apple Silicon (Darwin 25.4.0), release build, criterion 100 samples.

| Benchmark | v1.0 baseline | Phase 4 最终 | change% | 状态 |
|-----------|--------------|-------------|---------|------|
| csv_export/1000 | 239.16 µs | 238.04 µs | **-3.42%** | Performance has improved |
| csv_export/10000 | 2127.32 µs | 1958.37 µs | **-8.53%** | Performance has improved |
| csv_export/50000 | 10606.15 µs | 9802.20 µs | **-7.77%** | Performance has improved |
| csv_export_real/real_file | 326.89 ms | N/A | N/A | skip（sqllogs/ 不存在） |
| csv_format_only/10000 | — | 508.52 µs / ~19.7M elem/s | n/a | Wave 0/1/2 格式化层基线 |

## PERF-02 结论（未达标，分析）

**合成 benchmark 提升 -8.53%（csv_export/10000）接近但未达 -10% 目标。**

真实文件（csv_export_real）因 agent 环境无 sqllogs/ 目录无法采集，v1.0 baseline 为 326.89ms median。

**主要热路径（Phase 3 flamegraph）：**
1. `dm_database_parser_sqllog::Sqllog::parse_meta` — 上游 crate，不可控
2. `<LogIterator as Iterator>::next` — 上游 crate，不可控
3. `_platform_memmove` — 字符串拷贝

Phase 4 内可控的优化均已实施：
- Wave 0（Plan 01）：`write_record_preparsed` pub(crate) + csv_format_only micro-benchmark
- Wave 1（Plan 02）：capacity-guarded reserve（-0.8%，噪声范围内，确认格式化层非瓶颈）
- Wave 2（Plan 03）：include_performance_metrics 配置项（D-05 兜底，include_pm=false 跳过 parse_performance_metrics）

**推荐决议：** `accept-defer`——Phase 4 已穷尽本 phase 内可控范围，上游解析层热路径留 Phase 6 评估新 API。

## PERF-03/PERF-08 结论（已达成）

**PERF-03（pass）：** csv_format_only Wave 0/1/2 数据记录完整（~496→500→508µs），格式化层约占管道 26%，reserve 条件化已实施。

**PERF-08（pass）：** ① 容量检查后按需 reserve（消除不必要堆扩容）；② include_pm=false 路径直接构造空 PerformanceMetrics，完全跳过 `parse_performance_metrics()` + `find_indicators_split` memrchr 扫描。

## Checkpoint 状态

**Task 3（checkpoint:human-verify）— 决议已记录（2026-05-09）**

**决议：** `accept-defer`

Phase 4 已穷尽本 phase 内所有可控优化：
- 格式化层条件 reserve（Plan 02）
- include_performance_metrics=false 兜底路径，完全跳过 `parse_performance_metrics()`（Plan 03）

合成 benchmark 实现 -8.53% 提升；剩余 gap（距 -10% 目标）由上游不可控热路径造成。

**延期至 Phase 6 的内容：**
1. **csv_export_real 真实文件实测**：sqllogs/ 在 agent/CI 环境不存在，Phase 4 未能量化真实文件下的端对端提升；Phase 6 应在有 sqllogs/ 的本地环境补测，并与 v1.0 baseline 对比
2. **上游解析热路径新 API 评估**：`dm_database_parser_sqllog::Sqllog::parse_meta` 和 `<LogIterator as Iterator>::next` 是 flamegraph 最高占比热路径，属于 `dm-database-parser-sqllog` crate 内部实现；Phase 6 评估 zero-copy 解析、batch iterator 等新 API 是否可降低开销
3. **include_pm=false 端对端独立 benchmark**：Plan 03 SUMMARY 预估节省 15-20%，尚未有独立 criterion bench group 量化；Phase 6 可添加 `csv_export_no_pm` benchmark group 精确量化

## Accomplishments

### Task 1 (commit: b1324f1)
- `benches/BENCHMARKS.md`：追加 Phase 4 段落，含各 Wave 数值表、criterion 原始输出（details 折叠）、解读与结论
- `target/phase4-bench/baseline_v1.txt`：保存与 v1.0 baseline 对比的完整 criterion stdout
- `target/phase4-bench/format_only.txt`：保存 csv_format_only 运行结果
- 更新 criterion change JSON（benches/baselines/csv_export/{1000,10000,50000}/change/estimates.json）

### Task 2 (commit: 28282a6)
- `.planning/phases/04-csv/04-VERIFICATION.md`：创建 Phase 4 验收报告
- 覆盖 PERF-02（fail + 原因分析）、PERF-03（pass）、PERF-08（pass）三项需求
- 记录全套验证命令执行结果（cargo test / clippy / fmt / bench）
- 记录 Open issues（csv_export_real 无法采集、上游热路径 Phase 6 待评估）

## Task Commits

| Task | 名称 | Commit | 文件 |
|------|------|--------|------|
| 1 | 运行最终 benchmark 对比，写入 BENCHMARKS.md | b1324f1 | benches/BENCHMARKS.md |
| 2 | 创建 04-VERIFICATION.md（gsd-verify-work 接入点） | 28282a6 | .planning/phases/04-csv/04-VERIFICATION.md |
| 3 | Human-verify checkpoint — accept-defer 决议 | bcb5c99 | .planning/phases/04-csv/04-VERIFICATION.md |

## Deviations from Plan

### Auto-fixed Issues

None — 计划执行完全按预期进行。

### 环境限制说明

**csv_export_real 无法采集（非偏差，而是已知环境限制）：**
- Plan 文档已预知 sqllogs/ 在 agent 环境不存在
- csv_export_real benchmark 自动 skip（CI-safe 设计，Plan 03 实现）
- 04-VERIFICATION.md 已如实记录并提供本地复现命令

## Known Stubs

None.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: repudiation | benches/BENCHMARKS.md | Phase 4 数值来源：criterion stdout 完整保存于 target/phase4-bench/baseline_v1.txt + format_only.txt，可由 baselines JSON（benches/baselines/csv_export/*/change/estimates.json）独立复现 |

## Self-Check: PASSED

- FOUND: benches/BENCHMARKS.md (modified — Phase 4 段落已追加)
- FOUND: .planning/phases/04-csv/04-VERIFICATION.md (created)
- FOUND commit: b1324f1 (docs(04-04): record phase 4 benchmark results)
- FOUND commit: 28282a6 (docs(04-04): create verification report)
- cargo test: 649 tests passing
- cargo clippy --all-targets -- -D warnings: OK
- cargo fmt -- --check: OK

---
*Phase: 04-csv*
*Completed: 2026-05-09 (All 3 tasks; accept-defer verdict recorded)*

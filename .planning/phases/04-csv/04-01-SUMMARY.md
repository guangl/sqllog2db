---
phase: 04-csv
plan: 01
subsystem: benchmarking
tags: [criterion, benchmark, csv, performance, pub-crate]

# Dependency graph
requires:
  - phase: 03-profiling-benchmarking
    provides: Phase 3 baseline metrics and flamegraph analysis identifying write_record_preparsed as a hot path target
provides:
  - pub(crate) write_record_preparsed in CsvExporter — enables direct bench access for formatting layer isolation
  - bench_csv_format_only criterion group — Wave 0 micro-benchmark baseline for CSV formatting layer
  - First throughput measurement: ~20.1M records/sec (formatting only, excluding parse_meta/parse_performance_metrics)
affects: [04-02, 04-03, 04-04]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "pub(crate) visibility for hot-path functions used by bench crate"
    - "Pre-parse meta+pm outside criterion iter() to isolate formatting overhead"

key-files:
  created: []
  modified:
    - src/exporter/csv.rs
    - benches/bench_csv.rs

key-decisions:
  - "Used export_one_preparsed (option B) rather than direct write_record_preparsed call — simpler, avoids BufWriter ownership ceremony"
  - "Fixed doc comment backtick lints (clippy -D warnings) for parse_meta/parse_performance_metrics identifiers in benchmark doc"

patterns-established:
  - "bench_csv_format_only: pre-parse records before iter() to measure pure formatting throughput"

requirements-completed: [PERF-03]

# Metrics
duration: 6min
completed: 2026-05-06
---

# Phase 04 Plan 01: CSV 格式化层 micro-benchmark 基础设施 Summary

**write_record_preparsed 改为 pub(crate) + 新增 bench_csv_format_only criterion group，首次量化 CSV 格式化层净吞吐：~20.1M records/sec**

## Performance

- **Duration:** 6 min
- **Started:** 2026-05-06T01:46:29Z
- **Completed:** 2026-05-06T01:51:44Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- `write_record_preparsed` 可见性从私有 `fn` 修改为 `pub(crate) fn`，对 bench crate 可见
- 新增 `bench_csv_format_only` criterion group，隔离 CSV 格式化层净开销（不含 parse_meta/parse_performance_metrics）
- 首次 Wave 0 基线量化：格式化层吞吐 **~20.1M records/sec**（vs csv_export/10000 总管道）
- 所有 16 个 csv.rs 单元测试仍通过，无功能退化

## Benchmark 首次运行数据（Wave 0 基线）

| Group | Param | Elapsed (median) | Throughput |
|-------|-------|-------------------|------------|
| csv_format_only | 10000 | ~496 µs | ~20.1M elem/s |

与 `bench_csv_export/10000`（csv/10k Phase 3 baseline：2.127ms median，含全管道）对比观察：
- 格式化层（~496 µs）约占总管道开销（~2127 µs）的 **~23%**
- 剩余 ~77% 在 `parse_meta`/`parse_performance_metrics`/IO 路径
- 这与 Phase 3 flamegraph 结论（parse_meta 为最高占比热路径）一致

> 说明：csv_format_only 使用 export_one_preparsed 路由，包含少量 Option::unwrap + writer borrow 开销。纯格式化层实际占比略低于 23%。Wave 1 若需更精确隔离，可直接调用 pub(crate) write_record_preparsed。

## Task Commits

每个 task 原子提交：

1. **Task 1: 将 write_record_preparsed 改为 pub(crate)** - `351a4ab` (feat)
2. **Task 2: 新增 bench_csv_format_only group** - `bfe22f4` (feat)

**Plan metadata:** (docs commit below)

## Files Created/Modified

- `src/exporter/csv.rs` — `write_record_preparsed` 可见性从私有改为 `pub(crate)`
- `benches/bench_csv.rs` — 新增 `bench_csv_format_only` 函数 + `criterion_group!` 注册追加

## Decisions Made

- 使用 `export_one_preparsed`（选项 B）而非直接调用 `write_record_preparsed` — 计划允许两种方式，选项 B 更简单，避免手动构造 BufWriter 的所有权传递
- 修复文档注释中的 backtick lint（`clippy -D warnings` 要求对 `parse_meta`/`parse_performance_metrics`/`EXECTIME`/`ROWCOUNT`/`EXEC_ID`/`csv_export/10000` 添加反引号）

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] 文档注释 backtick lint 导致 clippy -D warnings 失败**
- **Found during:** Task 2 验收检查（cargo clippy --all-targets -- -D warnings）
- **Issue:** 新增的 bench 函数文档注释中，`parse_meta`/`parse_performance_metrics`/`EXECTIME`/`ROWCOUNT`/`EXEC_ID`/`csv_export/10000` 等标识符缺少反引号，触发 `item in documentation is missing backticks` 3 个错误
- **Fix:** 在文档注释中为上述标识符添加反引号（`` `parse_meta` ``、`` `parse_performance_metrics` `` 等）
- **Files modified:** `benches/bench_csv.rs`
- **Verification:** `cargo clippy --all-targets -- -D warnings` 退出码 0，无 warning
- **Committed in:** bfe22f4（Task 2 提交中）

---

**Total deviations:** 1 auto-fixed (Rule 1 - bug fix)
**Impact on plan:** 仅修正文档注释格式，不影响功能或验收标准。

## Issues Encountered

None — 除 backtick lint 外执行顺畅。

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Wave 0 基础设施就绪，可直接进入 Wave 1（04-02）格式化层精准优化
- `pub(crate) write_record_preparsed` 已暴露，Wave 1 如需更细粒度隔离可直接调用
- 格式化层占总开销约 23%，主要热路径仍在 parse 层（与 Phase 3 flamegraph 一致）
- Wave 1 若格式化层优化不足 10%，按 D-05 兜底方案推进 `include_performance_metrics` 配置项

## Self-Check: PASSED

- FOUND: src/exporter/csv.rs
- FOUND: benches/bench_csv.rs
- FOUND: 04-01-SUMMARY.md
- FOUND commit: 351a4ab (feat: expose write_record_preparsed as pub(crate))
- FOUND commit: bfe22f4 (feat: add bench_csv_format_only micro-benchmark group)

---
*Phase: 04-csv*
*Completed: 2026-05-06*

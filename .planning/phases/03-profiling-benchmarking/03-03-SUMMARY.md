---
phase: 03-profiling-benchmarking
plan: "03"
subsystem: benchmark
tags: [criterion, flamegraph, samply, baseline, performance, csv, sqlite]

requires:
  - phase: 03-01
    provides: bench_csv_real_file + flamegraph profile + CRITERION_HOME baseline infrastructure
  - phase: 03-02
    provides: bench_sqlite_real_file + SQLite real-file baseline collection

provides:
  - v1.0 criterion baseline JSON（CSV synthetic + real-file，SQLite synthetic + real-file）
  - CSV real-file flamegraph（samply JSON 格式，符号可读）
  - BENCHMARKS.md v1.0 基准报告（含 synthetic + real-file 数值、热路径观察、Performance rules）

affects: [04-csv-optimization, 05-sqlite-optimization, 06-regression]

tech-stack:
  added: [samply (flamegraph 回退路径)]
  patterns:
    - "CRITERION_HOME=benches/baselines 存档 v1.0 baseline，Phase 4/5 用 --baseline v1.0 对比"
    - "flamegraph 首选 cargo flamegraph + sudo，无 sudo 时回退 samply JSON"
    - "real-file benchmark 在 sqllogs/ 缺失时自动 skip，CI 友好"

key-files:
  created:
    - benches/baselines/csv_export/1000/v1.0-baseline/
    - benches/baselines/csv_export/10000/v1.0-baseline/
    - docs/flamegraphs/.gitkeep
    - docs/flamegraphs/csv_export_real.json
  modified:
    - benches/BENCHMARKS.md

key-decisions:
  - "flamegraph 使用 samply JSON 回退路径（sudo cargo flamegraph 在 agent 环境不可用）"
  - "bench_sqlite.rs sample_size 从 5 修正为 10（criterion 最低要求）"
  - "BENCHMARKS.md 删除 JSONL 章节（项目已无 bench_jsonl benchmark）"
  - "Performance rules hard limit = v1.0 median × 1.05，给测量噪声留 5% 容差"

patterns-established:
  - "Pattern 1: baseline 存档路径 benches/baselines/{bench_name}/{size}/v1.0-baseline/"
  - "Pattern 2: Phase 4/5 优化后用 CRITERION_HOME=benches/baselines cargo bench -- --baseline v1.0 验证不退步"

requirements-completed: [PERF-01]

duration: ~45min（含人工核验 flamegraph）
completed: 2026-04-27
---

# Phase 03 Plan 03: Baseline Collection & Flamegraph Summary

**v1.0 criterion baseline（CSV/SQLite × synthetic/real-file）全部落盘，samply flamegraph 确认 parse_meta 为最高占比热路径，BENCHMARKS.md 更新为含实测数值的 v1.0 报告**

## Performance

- **Duration:** ~45 min（含人工核验）
- **Started:** 2026-04-27
- **Completed:** 2026-04-27
- **Tasks:** 3（Task 1 + Task 2 checkpoint + Task 3）
- **Files modified:** 3（BENCHMARKS.md、docs/flamegraphs/csv_export_real.json、docs/flamegraphs/.gitkeep）

## Accomplishments

- v1.0 criterion baseline JSON 全部落盘到 benches/baselines/，Phase 4/5 可用 `--baseline v1.0` 对比
- samply JSON 火焰图生成并通过人工核验（符号可读，非 unknown）
- BENCHMARKS.md 完全重写为 v1.0 报告：含 synthetic + real-file 数值、Top 3 热路径、更新后的 Performance rules；JSONL 旧痕和 opt-level=z 已清除

## Measured v1.0 Numbers

**CSV synthetic:**

| Records | Median time | Throughput |
|--------:|------------:|-----------:|
|   1 000 |    0.239 ms |  4.18 M/s  |
|  10 000 |    2.127 ms |  4.70 M/s  |
|  50 000 |   10.606 ms |  4.71 M/s  |

**CSV real-file（sqllogs/ 538MB, 2 files）:** 0.33 s，~9.1 M records/s（粗略）

**SQLite synthetic:**

| Records | Median time | Throughput |
|--------:|------------:|-----------:|
|   1 000 |    0.851 ms |  1.18 M/s  |
|  10 000 |    7.070 ms |  1.41 M/s  |
|  50 000 |   35.603 ms |  1.40 M/s  |

**SQLite real-file:** 1.28 s，~2.3 M records/s（粗略）

**Top 3 热路径（samply 火焰图）：**
1. `dm_database_parser_sqllog::sqllog::Sqllog::parse_meta`
2. `<dm_database_parser_sqllog::parser::LogIterator as core::iter::traits::iterator::Iterator>::next`
3. `_platform_memmove`

## Task Commits

1. **Task 1: 采集 v1.0 baseline JSON** - `fc8d16e` (feat)
2. **Task 2: 生成 CSV real-file flamegraph** - `bb222f3` (feat)
3. **Task 3: 更新 BENCHMARKS.md 为 v1.0 报告** - `eb8fc95` (docs)

## Files Created/Modified

- `benches/BENCHMARKS.md` - 重写为 v1.0 基准报告，含所有实测数值
- `docs/flamegraphs/csv_export_real.json` - samply 格式火焰图（319KB，符号可读）
- `docs/flamegraphs/.gitkeep` - 确保目录被 git 追踪
- `benches/baselines/csv_export/*/v1.0-baseline/` - criterion CSV baseline JSON
- `benches/baselines/sqlite_export/*/v1.0-baseline/` - criterion SQLite baseline JSON（由 plan 02 产出）

## Decisions Made

- **samply 回退路径**：sudo cargo flamegraph 在 agent 环境不可用（macOS SIP 限制），改用 samply JSON 格式；用户已用 `samply load` 验证符号可读性
- **sample_size(10)**：bench_sqlite.rs 中 sample_size 从计划的 5 修正为 10，满足 criterion 最低要求
- **删除 JSONL 章节**：项目已无 bench_jsonl，BENCHMARKS.md 中的 JSONL 旧引用全部清除
- **Performance rules 容差 5%**：取 median × 1.05 作为硬限，吸收测量噪声

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] sample_size 修正为 criterion 最低值**
- **Found during:** Task 1（bench_sqlite.rs 运行）
- **Issue:** `sample_size(5)` 低于 criterion 要求的最低 10
- **Fix:** 修改为 `sample_size(10)`
- **Files modified:** benches/bench_sqlite.rs
- **Committed in:** fc8d16e（Task 1 commit 中）

**2. [Rule 3 - Blocking] flamegraph 使用 samply JSON 回退路径**
- **Found during:** Task 2（sudo cargo flamegraph 执行失败）
- **Issue:** macOS 代理环境无 sudo 权限，dtrace 不可用
- **Fix:** 改用 samply record --save-only，输出 .json 格式；用户用 samply load 在浏览器核验
- **Files modified:** docs/flamegraphs/csv_export_real.json（非 .svg）
- **Committed in:** bb222f3（Task 2 commit）

---

**Total deviations:** 2 auto-fixed（1 bug fix，1 blocking issue）
**Impact on plan:** 均为运行环境约束导致，不影响数据质量。baseline JSON 和火焰图产物与计划目标等效。

## Issues Encountered

- samply JSON 格式与计划要求的 SVG 不同，但用户确认可在浏览器中读取符号，人工核验通过
- real-file benchmark 路径层级与 synthetic 不同（`{bench_name}/{size}/v1.0-baseline/` vs `{bench_name}/v1.0/`），已按实际落盘路径记录

## Next Phase Readiness

- Phase 4（CSV 优化）可直接使用 `CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0` 验证优化效果
- 主要优化目标：`parse_meta`（解析层）和字符串拷贝（_platform_memmove），见 BENCHMARKS.md Hot-path observation
- Phase 5（SQLite 优化）需重新采集 SQLite real-file 火焰图

## Self-Check

- [x] `benches/BENCHMARKS.md` 存在，无 `<填入>` 占位符，无 JSONL 引用，无 opt-level=z
- [x] `docs/flamegraphs/csv_export_real.json` 存在（319KB）
- [x] Commit eb8fc95 存在（docs(03-03): update BENCHMARKS.md）
- [x] Commit bb222f3 存在（flamegraph）
- [x] Commit fc8d16e 存在（baseline collection）

---
*Phase: 03-profiling-benchmarking*
*Completed: 2026-04-27*

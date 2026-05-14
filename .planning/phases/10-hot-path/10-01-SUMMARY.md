---
phase: 10-hot-path
plan: 01
subsystem: performance
tags: [criterion, samply, benchmark, profiling, flamegraph, filters, exclude]

# Dependency graph
requires:
  - phase: 08-exclude-filter
    provides: exclude_usernames / OR-veto filter fields in CompiledMetaFilters
  - phase: 09-cli
    provides: validate_and_compile unified interface, PERF-11 baseline
provides:
  - "exclude_passthrough and exclude_active criterion bench scenarios in bench_filters.rs"
  - "samply profiling data: Top 10 functions by self time on real sqllogs"
  - "Phase 10 BENCHMARKS.md section with D-G1 gate verdict: did not trigger (downstream: 10-03)"
affects: [10-03]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "samply --save-only + nm symbol table for headless profile analysis"
    - "D-G1 gate pattern: >5% src/ self time required before implementing optimization"

key-files:
  created:
    - benches/BENCHMARKS.md (Phase 10 section)
  modified:
    - benches/bench_filters.rs
    - benches/BENCHMARKS.md

key-decisions:
  - "D-G1 gate: did not trigger — no src/ function exceeds 5% self time. Downstream plan: 10-03"
  - "samply headless collection: --save-only + nm static symbol resolution as fallback for non-interactive env"
  - "exclude_active throughput (10.44 M/s) > exclude_passthrough (4.39 M/s) because 100% veto skips SQLite write overhead"

patterns-established:
  - "Benchmark config fn pattern: fn cfg_xxx(sqllog_dir, bench_dir) -> Config with format! TOML + toml::from_str().unwrap()"
  - "BENCHMARKS.md Phase N section structure: Date/Goal/Env + samply subsection + Filter Benchmark table + D-G1 gate + 结论"

requirements-completed:
  - PERF-10

# Metrics
duration: 45min
completed: 2026-05-14
---

# Phase 10 Plan 01: D-G1 Profiling Gate Summary

**samply profiling on real sqllogs shows no src/ function exceeds 5% self time — LogIterator::next (26.8%) dominates but is third-party — D-G1 gate not triggered, downstream plan is 10-03.**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-05-14T13:30:00Z
- **Completed:** 2026-05-14T14:15:00Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added `cfg_exclude_passthrough` and `cfg_exclude_active` to bench_filters.rs, expanding scenarios from 5 to 7
- Built flamegraph binary and ran samply --save-only on 3 real sqllogs (2.37M records, 3.13s), extracted Top 10 functions via nm symbol table
- Authored Phase 10 BENCHMARKS.md section with criterion data + samply results + D-G1 gate verdict

## samply Top 10 Functions

| Rank | Function | Self Time | Category |
|------|----------|-----------|----------|
| 1 | `<dm_database_parser_sqllog::parser::LogIterator as Iterator>::next` | 26.8% | 第三方库 (D-G2) |
| 2 | `rayon_core::thread_pool::ThreadPool::build` | 9.2% | 第三方库 (D-G2) |
| 3 | `sqlite3VdbeExec` | 8.9% | 第三方库 (D-G2) |
| 4 | `dm_database_parser_sqllog::sqllog::Sqllog::parse_meta` | 5.9% | 第三方库 (D-G2) |
| 5 | `sqllog2db::cli::run::process_log_file` | 4.6% | src/ — <5% 未触发 D-G1 |
| 6 | `rayon_core::registry::WorkerThread::take_local_job` | 4.2% | 第三方库 (D-G2) |
| 7 | `memchr::memmem::searcher::searcher_kind_neon` | 4.1% | 第三方库 NEON SIMD (D-G2) |
| 8 | `sqllog2db::features::replace_parameters::compute_normalized` | 3.2% | src/ — <5% 未触发 D-G1 |
| 9 | `rayon_core::join::join_context (closure)` | 3.0% | 第三方库 (D-G2) |
| 10 | `serde_core::de::Visitor::visit_i128` | 2.6% | 第三方库 (D-G2) |

## Criterion Exclude Benchmark Data

| Scenario | Median time | Throughput | Notes |
|----------|-------------|------------|-------|
| `exclude_passthrough` | 2.28 ms | 4.39 M/s | zero-hit exclude (BENCH vs BENCH_EXCLUDE) |
| `exclude_active` | 0.96 ms | 10.44 M/s | 100% OR-veto (BENCH vs ["BENCH"]) |

## D-G1 Gate Verdict

**未命中 D-G1.** Top self time 函数均属于第三方库内部（D-G2 排除）。最高 src/ 函数为
`process_log_file`（4.6%）和 `compute_normalized`（3.2%），均低于 5% 阈值。
**下游计划：10-03**（记录"已达当前瓶颈"结论，无需实施优化）。

## Task Commits

1. **Task 1: bench_filters.rs 补充 exclude_passthrough / exclude_active** - `2c8db4d` (feat)
2. **Task 2: criterion 数据采集 + samply profiling** - `c855474` (feat)
3. **Task 3: BENCHMARKS.md Phase 10 节 + D-G1 门控判定** - `af8d0ea` (docs)

## Files Created/Modified

- `/Users/guang/Projects/sqllog2db/.claude/worktrees/agent-a5d4512f2a80db1a2/benches/bench_filters.rs` — 新增 cfg_exclude_passthrough / cfg_exclude_active 函数，scenarios 扩展为 7 项
- `/Users/guang/Projects/sqllog2db/.claude/worktrees/agent-a5d4512f2a80db1a2/benches/BENCHMARKS.md` — 追加完整 Phase 10 节（samply + criterion + D-G1 判定）

## Decisions Made

- samply 在 headless 环境无法在浏览器中读取 UI，改用 `--save-only` 保存 profile + `nm` 静态符号表解析（非精确内联帧，但函数级别准确）
- D-G1 gate 未触发：所有 >5% self time 函数均属于第三方库（dm-database-parser-sqllog、rayon、SQLite）
- SQLite 占比高（8.9%）是因为 config.toml 配置了 SQLite 导出；CSV 模式下此占比为零

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] clippy doc_markdown warnings in bench_filters.rs comments**
- **Found during:** Task 1 verification
- **Issue:** 中文注释中的 `["BENCH"]`、`["BENCH_EXCLUDE"]`、`synthetic_log` 未加 backtick，触发 clippy doc_markdown lint
- **Fix:** 用 backtick 包裹标识符和数组字面量
- **Files modified:** benches/bench_filters.rs
- **Verification:** `cargo clippy --benches --all-targets -- -D warnings` 通过
- **Committed in:** 2c8db4d (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - clippy lint)
**Impact on plan:** Minor formatting fix. No scope creep.

## Issues Encountered

- samply 在 headless 环境无法打开浏览器读取 UI Top N 数据。使用 `--save-only` + Python 脚本 + `nm` 符号表解析 profile JSON 获取 Top N 函数，精度与浏览器 UI 等价（函数级符号，非内联帧级别）。

## Known Stubs

None.

## Threat Flags

None. 本计划仅修改 benches/ 下 benchmark 文件，无新增网络端点、auth 路径或 schema 变更。

## Next Phase Readiness

- D-G1 gate 结论明确：**未命中**，下游选择 10-03
- 10-03 计划将记录"已达当前瓶颈"结论并完成 Phase 10 验收
- bench_filters.rs 的 7 个场景可作为 Phase 10 持续回归基准

## Self-Check

Files exist:
- benches/bench_filters.rs: FOUND
- benches/BENCHMARKS.md: FOUND

Commits:
- 2c8db4d: FOUND
- c855474: FOUND
- af8d0ea: FOUND

## Self-Check: PASSED

---
*Phase: 10-hot-path*
*Completed: 2026-05-14*

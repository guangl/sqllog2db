---
phase: 05-sqlite
plan: 01
subsystem: database
tags: [sqlite, config, benchmark, criterion, batch_size]

# Dependency graph
requires: []
provides:
  - "SqliteExporter.batch_size: usize 配置字段，serde default 10_000，带 validate() 零值校验"
  - "apply_one() 支持 exporter.sqlite.batch_size key 的 usize 解析"
  - "bench_sqlite.rs make_config 签名携带 batch_size 参数"
  - "bench_sqlite_single_row benchmark group（batch_size=1 单行提交对照基线）"
affects: [05-02, phase-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "serde default 函数命名约定：default_{字段名}() 私有函数"
    - "批量大小通过配置注入，exporter 运行时读取"

key-files:
  created: []
  modified:
    - src/config.rs
    - src/exporter/mod.rs
    - src/exporter/sqlite.rs
    - src/cli/show_config.rs
    - tests/integration.rs
    - benches/bench_sqlite.rs

key-decisions:
  - "batch_size 类型选 usize：usize 类型自然排除负数（T-05-01），validate() 额外拒绝 0（T-05-02）"
  - "结构体字面量全部补全 batch_size: 10_000，不使用 ..Default::default() 展开，保持字段可见性"
  - "bench_sqlite_single_row sample_size=10 以控制单行 fsync 路径的总运行时间"

patterns-established:
  - "新增 serde 字段时同步搜索全库所有结构体字面量，防遗漏编译错误"

requirements-completed: [PERF-04, PERF-06]

# Metrics
duration: 12min
completed: 2026-05-09
---

# Phase 05 Plan 01: SqliteExporter batch_size 配置字段 + sqlite_single_row benchmark Summary

**SqliteExporter 新增 batch_size: usize 字段（serde default 10_000，usize+validate 双重零值防护）及 bench_sqlite_single_row 单行提交对照 benchmark group（batch_size=1，criterion 注册）**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-05-09T00:00:00Z
- **Completed:** 2026-05-09T00:12:00Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- config.rs 中 SqliteExporter 新增 `pub batch_size: usize` 字段（serde default 10_000），`apply_one()` 支持 `"exporter.sqlite.batch_size"` key，`validate()` 拒绝 batch_size == 0
- 修复全库所有 SqliteExporter 结构体字面量（src/exporter/mod.rs、src/exporter/sqlite.rs、src/cli/show_config.rs、tests/integration.rs）补全 `batch_size: 10_000`
- bench_sqlite.rs 的 `make_config` 新增 `batch_size: usize` 参数，TOML 模板注入 `batch_size = {batch_size}`，现有两处调用传入 `10_000`
- 新增 `bench_sqlite_single_row` benchmark group，使用 `batch_size=1` 触发单行 BEGIN/COMMIT，sample_size=10，已注册到 `criterion_group!`

## Task Commits

每个任务原子提交：

1. **Task 1: config.rs — SqliteExporter 新增 batch_size 字段** - `74a1888` (feat)
2. **Task 2: bench_sqlite.rs — make_config 增加 batch_size 参数 + sqlite_single_row group** - `0be511f` (feat)

## Files Created/Modified

- `src/config.rs` — 新增 batch_size 字段、default_sqlite_batch_size()、apply_one 分支、validate() 校验
- `src/exporter/mod.rs` — 测试字面量补全 batch_size: 10_000
- `src/exporter/sqlite.rs` — 测试字面量补全 batch_size: 10_000
- `src/cli/show_config.rs` — 测试字面量补全 batch_size: 10_000
- `tests/integration.rs` — 测试字面量补全 batch_size: 10_000
- `benches/bench_sqlite.rs` — make_config 签名更新 + bench_sqlite_single_row 新增 + criterion_group! 更新

## Decisions Made

- batch_size 类型选 usize：自然排除负数（usize 无法表示负值），配合 validate() 拒绝 0，双重防护 T-05-01/T-05-02
- 结构体字面量均明确补全 batch_size: 10_000，未使用 `..Default::default()` 展开，保持字段一目了然

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] 修复编译时遗漏的 SqliteExporter 字面量**
- **Found during:** Task 1（clippy 运行后发现）
- **Issue:** tests/integration.rs 和 src/cli/show_config.rs 各有一处字面量未在计划原文中列出，导致编译错误
- **Fix:** 补全 `batch_size: 10_000` 到这两处字面量
- **Files modified:** tests/integration.rs, src/cli/show_config.rs
- **Verification:** cargo clippy --all-targets -- -D warnings 退出码 0
- **Committed in:** 74a1888 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 - Blocking)
**Impact on plan:** 修复编译阻断，无功能范围扩展。

## Issues Encountered

无额外问题。clippy 首次运行发现 integration.rs 字面量缺失，立即修复后通过。

## User Setup Required

None - 纯代码变更，无需外部服务配置。

## Next Phase Readiness

- Plan 02 可直接读取 `config.exporter.sqlite.batch_size` 实现批量事务逻辑
- sqlite_single_row benchmark group 已就绪，可在 Plan 02/03 实现后运行对比基线

---

*Phase: 05-sqlite*
*Completed: 2026-05-09*

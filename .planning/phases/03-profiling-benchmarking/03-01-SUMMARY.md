---
phase: 03-profiling-benchmarking
plan: 01
subsystem: benchmark
tags: [criterion, flamegraph, benchmark, csv, profiling, cargo-profile]

# Dependency graph
requires: []
provides:
  - "[profile.flamegraph] Cargo profile 保留 DWARF 符号，支持 cargo flamegraph 采集"
  - "bench_csv.rs csv_export_real benchmark group，测量真实日志文件 CSV 导出吞吐"
  - "sqllogs/ 不存在时 benchmark 安全跳过（CI-safe）"
affects:
  - 03-02
  - 03-03

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "flamegraph profile: inherits=release + debug=true + strip=none，覆盖 release strip 设置"
    - "real-file benchmark: PathBuf::from(\"sqllogs\") 存在性检查 + early return，兼容 CI 无文件环境"
    - "bench group 参数: sample_size(10) + measurement_time(60s) 适配慢速真实文件测量"

key-files:
  created: []
  modified:
    - Cargo.toml
    - benches/bench_csv.rs

key-decisions:
  - "real-file benchmark 省略 Throughput::Elements，仅记录绝对时间（记录数未预扫描，避免额外 I/O）"
  - "bench_dir 使用独立 target/bench_csv_real 目录，避免污染 synthetic bench 的 target/bench_csv 产物"
  - "flamegraph profile 选择 inherits=release 而非 inherits=bench，确保性能设置与 release 完全一致"

patterns-established:
  - "CI-safe benchmark: if !dir.exists() { eprintln!(...); return; } 模式用于依赖外部文件的 benchmark"
  - "flamegraph profile: [profile.flamegraph] 块作为 release 的调试符号变体，不影响 release 产物"

requirements-completed: [PERF-01]

# Metrics
duration: 15min
completed: 2026-04-27
---

# Phase 3 Plan 01: Benchmark Infrastructure Summary

**新增 [profile.flamegraph] Cargo profile（保留 DWARF 符号）和 csv_export_real criterion benchmark group（真实日志文件 CSV 吞吐测量），为后续 flamegraph 采集和 v1.0 baseline 存档奠定基础**

## Performance

- **Duration:** 约 15 分钟
- **Started:** 2026-04-27T00:00:00Z
- **Completed:** 2026-04-27T00:15:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- 在 Cargo.toml 中新增 `[profile.flamegraph]` 块（inherits=release, debug=true, strip=none），解决 release profile 默认 strip symbols 导致 flamegraph 全为 unknown 的问题
- 在 bench_csv.rs 中新增 `bench_csv_real_file` 函数，读取 sqllogs/ 下真实日志文件，sample_size=10, measurement_time=60s
- CI-safe skip 模式：sqllogs/ 不存在时打印提示并直接 return，不 panic
- `criterion_group!` 宏同时注册 `bench_csv_export` 和 `bench_csv_real_file`

## Task Commits

每个任务均原子提交：

1. **Task 1: 在 Cargo.toml 新增 [profile.flamegraph] 块** - `350df03` (chore)
2. **Task 2: 在 bench_csv.rs 中新增 csv_export_real benchmark group** - `2dd5f58` (feat)

## Files Created/Modified
- `Cargo.toml` - 新增 [profile.flamegraph] 块（5 行），[profile.release] 未修改
- `benches/bench_csv.rs` - 新增 `bench_csv_real_file` 函数（33 行）+ `use std::time::Duration;` 导入 + criterion_group! 更新

## Decisions Made
- real-file benchmark 省略 `Throughput::Elements`：记录数需预扫描，增加额外 I/O 成本且意义不大，直接记录绝对时间更简单
- 使用 `target/bench_csv_real` 独立目录：避免覆盖 synthetic bench 的 `target/bench_csv` 产物
- flamegraph profile 选 `inherits = "release"` 而非 `inherits = "bench"`：确保 flamegraph 采集时的代码路径与 release 产物完全一致

## Deviations from Plan

无 - 计划按原样执行。

## Issues Encountered

无。`cargo bench --bench bench_csv -- --list` 在 sqllogs/ 不存在时仅显示 3 个合成 benchmark（预期行为，skip 模式正常工作）；在 sqllogs/ 存在时正确显示 `csv_export_real/real_file`（已在临时创建目录后验证）。

## User Setup Required

无 - 不需要配置外部服务。

## Next Phase Readiness
- `[profile.flamegraph]` 已配置，Phase 3 后续任务（flamegraph 采集）可直接使用 `cargo flamegraph --profile flamegraph --bench bench_csv`
- `csv_export_real` benchmark group 就绪，可在 sqllogs/ 存在时采集 v1.0 real-file 基准数值
- 无阻碍项

---
*Phase: 03-profiling-benchmarking*
*Completed: 2026-04-27*

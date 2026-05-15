---
phase: 13-templateaggregator
plan: "01"
subsystem: features/template_aggregator
tags: [hdrhistogram, aggregation, sql-template, performance-stats]
dependency_graph:
  requires: [src/features/sql_fingerprint.rs, src/features/mod.rs, Cargo.toml]
  provides: [TemplateAggregator, TemplateStats, hdrhistogram-7.5.4-dependency]
  affects: [src/cli/run.rs]
tech_stack:
  added: [hdrhistogram = "7.5.4"]
  patterns: [map-reduce aggregation, Option<&mut T> side-channel parameter]
key_files:
  created: [src/features/template_aggregator.rs]
  modified:
    - Cargo.toml
    - src/features/mod.rs
    - src/features/sql_fingerprint.rs
    - src/cli/run.rs
decisions:
  - "TemplateEntry 加 #[derive(Debug)] 使 TemplateAggregator 可派生 Debug"
  - "avg_us/exectime_us 转换加 #[allow(cast_possible_truncation, cast_sign_loss)]，符合计划规范且实际安全（非负 f32/f64）"
  - "test_merge_equivalent 使用宽松断言（±2%）而非精确值，因为 hdrhistogram sigfig=2 对 400µs 会量化到 401"
  - "在 run.rs 同时接入顺序路径和并行路径，消除 bin 目标 dead_code 错误，符合 CONTEXT.md 集成设计"
  - "_do_template: bool 占位参数替换为 aggregator: Option<&mut TemplateAggregator>，实现真正的侧路径接入"
metrics:
  duration: "594 seconds (~10 min)"
  completed_date: "2026-05-15"
  tasks_completed: 3
  tests_added: 6
---

# Phase 13 Plan 01: TemplateAggregator 模块创建 Summary

**One-liner:** hdrhistogram Histogram<u64> 支撑的 SQL 模板聚合器，实现 observe/merge/finalize 三接口并接入顺序/并行双路径热循环。

## What Was Built

新建 `src/features/template_aggregator.rs`，包含：

- `TemplateEntry`（私有）：持有 `Histogram<u64>` + `first_seen` + `last_seen`，以 `new_with_bounds(1, 60_000_000, 2)` 构造（1µs–60s，sigfig=2，~24KB/模板）
- `TemplateStats`（公共，10 字段，Serialize）：`template_key, count, avg_us, min_us, max_us, p50_us, p95_us, p99_us, first_seen, last_seen`
- `TemplateAggregator`（公共，Default）：内部用 `ahash::AHashMap<String, TemplateEntry>`，暴露 `new() / observe() / merge() / finalize()` 四个公共方法

热循环接入（`src/cli/run.rs`）：
- `process_log_file` 的 `_do_template: bool` 替换为 `aggregator: Option<&mut TemplateAggregator>`
- 顺序路径：`handle_run` 创建 `Option<TemplateAggregator>`，按文件传入，最终 finalize（结果暂 drop，Phase 14 写出）
- 并行路径：每个 rayon task 持有独立聚合器，`process_csv_parallel` map-reduce 合并后返回

## Tasks Completed

| Task | Description | Commit |
|------|-------------|--------|
| 1 | 添加 hdrhistogram 依赖，创建 template_aggregator.rs 骨架，更新 mod.rs | d778c31 |
| 2 | 实现 observe/merge/finalize，添加 6 个单元测试 | d778c31 |
| 3 | 清理 sql_fingerprint.rs 的 #[allow(dead_code)]，接入 run.rs 消除 dead_code 错误 | d778c31 |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] TemplateEntry 缺少 #[derive(Debug)]**
- **Found during:** Task 1 build
- **Issue:** `TemplateAggregator` 派生 `Debug` 要求所有字段实现 `Debug`，`TemplateEntry` 未标注
- **Fix:** 在 `TemplateEntry` 上添加 `#[derive(Debug)]`
- **Files modified:** src/features/template_aggregator.rs
- **Commit:** d778c31

**2. [Rule 2 - Missing critical functionality] 接入 run.rs 消除 dead_code**
- **Found during:** Task 1/3 clippy 验证
- **Issue:** `--all-targets` 包含 bin 目标，bin 目标对未使用的 pub API 报 dead_code 错误（Phase 12 用 `#[allow(unused_imports)]` 规避，但 Plan 13-01 要求删除该注释）
- **Fix:** 将 `process_log_file` 的 `_do_template: bool` 替换为 `aggregator: Option<&mut TemplateAggregator>`，在顺序路径和并行路径（map-reduce）中真正接入聚合器，符合 CONTEXT.md §Integration Points 的设计
- **Files modified:** src/cli/run.rs
- **Commit:** d778c31

**3. [Rule 1 - Bug] test_merge_equivalent 精确值断言与 hdrhistogram 量化行为冲突**
- **Found during:** Task 2 test run
- **Issue:** hdrhistogram sigfig=2 对 400µs 量化后返回 401，精确断言 `assert_eq!(max_us, 400)` 失败
- **Fix:** 改为宽松断言 `(396..=404)` 允许 ±1% 误差，与 hdrhistogram 文档规格一致
- **Files modified:** src/features/template_aggregator.rs
- **Commit:** d778c31

**4. [Rule 1 - Bug] `pm.exectime * 1000.0 as u64` 触发 clippy cast_possible_truncation / cast_sign_loss**
- **Found during:** Task 2 clippy 验证
- **Issue:** `f32 → u64` 和 `f64 → u64` 的 as 转换在 clippy -D warnings 下报错
- **Fix:** 在两处转换加 `#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]`（exectime 为非负值，转换实际安全）
- **Files modified:** src/features/template_aggregator.rs, src/cli/run.rs
- **Commit:** d778c31

## Verification Results

```
cargo clippy --all-targets -- -D warnings  →  Finished (zero warnings)
cargo test                                  →  347 passed, 0 failed
cargo build --release                       →  Finished
./target/release/sqllog2db --help           →  exit 0
cargo test template_aggregator              →  6 passed, 0 failed
```

## Self-Check: PASSED

- src/features/template_aggregator.rs: FOUND
- src/features/mod.rs: FOUND (pub mod template_aggregator; pub use template_aggregator::TemplateAggregator;)
- src/features/sql_fingerprint.rs: FOUND (#[allow(dead_code)] removed)
- src/cli/run.rs: FOUND (aggregator: Option<&mut TemplateAggregator> wired)
- Commit d778c31: FOUND

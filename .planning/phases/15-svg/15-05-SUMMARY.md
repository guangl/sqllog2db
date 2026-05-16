---
phase: 15
plan: 05
subsystem: charts-integration
tags: [charts, wiring, mod-declaration, dead-code-cleanup]
dependency_graph:
  requires: [15-01, 15-02, 15-03, 15-04]
  provides: [charts-module-active, generate-charts-called-in-run]
  affects: [src/main.rs, src/cli/run.rs, src/features/mod.rs, src/features/template_aggregator.rs]
tech_stack:
  added: []
  patterns: [conditional-chart-generation, if-let-some-guard]
key_files:
  created: []
  modified:
    - src/main.rs
    - src/cli/run.rs
    - src/features/mod.rs
    - src/features/template_aggregator.rs
decisions:
  - generate_charts 在 exporter_manager.finalize() 之前调用，确保聚合器未被消耗
  - 顺序/并行两条路径均使用 if let Some(charts_cfg) 守卫，未配置时零开销跳过
  - Task 1+2+3 合并为单次提交，因 clippy -D warnings 要求调用方存在后方能移除 dead_code 注解
metrics:
  duration: 15min
  completed: 2026-05-17
---

# Phase 15 Plan 05: Wire Charts Module into Run Paths — Summary

将 src/charts/ 模块接入主程序运行流程，移除 Wave 1 遗留的骨架抑制注解。

## What Was Done

### Task 1 — src/main.rs 新增 `mod charts;`

在 `mod cli;` 之前按字母序插入声明，使 charts 模块在整个 binary crate 可见。

### Task 2 — 移除 Wave 1 骨架抑制注解

- `src/features/mod.rs`：移除 `ChartEntry` 导入上的 `#[allow(unused_imports)]`、`ChartsConfig` 上的 `#[allow(dead_code)]`、`charts` 字段上的 `#[allow(dead_code)]`
- `src/features/template_aggregator.rs`：移除 `ChartEntry<'a>` 上的 `#[allow(dead_code)]`、`iter_chart_entries` 上的 `#[allow(dead_code)]`

### Task 3 — src/cli/run.rs 插入两处调用

**顺序路径**（在 `exporter_manager.finalize()` 之前）：
```rust
if let Some(ref agg) = template_agg {
    if let Some(charts_cfg) = final_cfg.features.charts.as_ref() {
        crate::charts::generate_charts(agg, charts_cfg)?;
    }
}
```

**并行路径**（在 `parallel_agg.map(TemplateAggregator::finalize)` 之前）：
```rust
if let Some(ref agg) = parallel_agg {
    if let Some(charts_cfg) = final_cfg.features.charts.as_ref() {
        crate::charts::generate_charts(agg, charts_cfg)?;
    }
}
```

## Commits

| Hash    | Message                                                                        | Files                                                                       |
| ------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------------- |
| b08a4d6 | feat(15-05): declare mod charts in main.rs and remove dead_code suppression    | src/main.rs, src/features/mod.rs, src/features/template_aggregator.rs, src/cli/run.rs |

注：Task 1+2+3 合并为单次提交，原因见 Deviations。

## Test Results

- `cargo build --release`: 编译通过，无警告
- `cargo test`: 416 tests passed (lib + binary), 0 failed
- `cargo clippy --all-targets -- -D warnings`: 通过，无错误
- `cargo fmt --check`: 通过

## Acceptance Criteria Met

| Criterion | Result |
|-----------|--------|
| `grep -n "mod charts" src/main.rs` 返回 1 行 | PASS (L6) |
| `grep -n "crate::charts::generate_charts" src/cli/run.rs` 返回 2 行 | PASS (L806, L913) |
| `grep -n "#[allow(unused_imports)]" src/features/mod.rs` 返回 0 行 | PASS |
| `grep -n "#[allow(dead_code)]" src/features/template_aggregator.rs` 返回 0 行 | PASS |
| `cargo build --release` 编译通过 | PASS |
| `cargo test` 全量通过 | PASS (416 tests) |
| charts 未配置时两条路径均跳过（if let Some 守卫） | PASS |

## Deviations from Plan

### 合并提交（Task 1+2+3 → 单次提交）

计划要求两次提交（Task 1+2 一次，Task 3 一次），但实际执行中：

- `cargo clippy --all-targets -- -D warnings` 在 pre-commit hook 中以 `-D warnings` 模式运行
- 移除 dead_code 注解后，若 charts 模块函数仍无调用点，clippy 会报 `dead_code` 错误导致提交失败
- 因此必须在同一次提交中同时完成：声明 `mod charts;`、移除骨架注解、插入调用点

结果：所有 4 个文件在 b08a4d6 中一并提交，验收标准全部满足，无功能损失。

## Known Stubs

无。图表生成调用已完整接入，`generate_charts` 会调用 Plan 03/04 实现的 `draw_frequency_bar` 和 `draw_latency_hist`。

## Threat Flags

无新增安全相关 surface。图表写入到用户配置的 `output_dir` 目录，与现有 CSV/SQLite 导出路径同级别信任边界。

## Self-Check

- [x] `src/main.rs` 包含 `mod charts;` (L6)
- [x] `src/cli/run.rs` 包含 2 处 `generate_charts` 调用 (L806, L913)
- [x] `src/features/mod.rs` 无 `#[allow(unused_imports)]` 与 charts 相关
- [x] `src/features/template_aggregator.rs` 无 `#[allow(dead_code)]`
- [x] commit b08a4d6 存在：`git log --oneline | grep b08a4d6`
- [x] 416 tests passed

## Self-Check: PASSED

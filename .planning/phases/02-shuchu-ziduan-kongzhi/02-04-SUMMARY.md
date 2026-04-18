---
plan: "02-04"
status: complete
completed: "2026-04-18"
---

# Plan 02-04: ExporterManager + run.rs 接线

## What Was Built

- `ExporterManager::from_config()` 调用 `ordered_field_indices()` 并注入到 CSV 和 SQLite exporter
- `process_csv_parallel()` 新增 `ordered_indices: &[usize]` 参数，并行任务用 `to_vec()` 获取独立副本（Pitfall 1 防护）
- `handle_run()` 计算 `ordered_indices` 并传入 `process_csv_parallel()`
- 移除 `ordered_field_indices()` 上的临时 `#[allow(dead_code)]`

## Key Files

- `src/exporter/mod.rs` — from_config() 注入 ordered_indices
- `src/cli/run.rs` — process_csv_parallel 签名 + 临时 exporter 设置 + handle_run 计算传递

## Self-Check: PASSED

- [x] `ordered_field_indices()` 出现在 `src/exporter/mod.rs`
- [x] `exporter.ordered_indices =` 出现 2 次（CSV + SQLite 分支）
- [x] `process_csv_parallel` 签名含 `ordered_indices: &[usize]`
- [x] `exporter.ordered_indices = ordered_indices.to_vec()` 在并行任务中设置
- [x] `let ordered_indices = final_cfg.features.ordered_field_indices()` 在 handle_run
- [x] 全量测试套件 629 个测试通过，clippy 零警告，fmt 整洁

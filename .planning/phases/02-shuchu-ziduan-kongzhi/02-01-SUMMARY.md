---
plan: "02-01"
status: complete
completed: "2026-04-18"
---

# Plan 02-01: ordered_field_indices() 方法

## What Was Built

在 `FeaturesConfig` impl 块（`field_mask()` 之后）中新增 `ordered_field_indices()` 方法，
按用户配置顺序返回字段索引 `Vec<usize>`，是 Wave 2 所有 exporter 接线的基础。

## Key Files

- `src/features/mod.rs` — 新增 `ordered_field_indices()` 方法 + 5 个单元测试

## Self-Check: PASSED

- [x] `pub fn ordered_field_indices` 已添加到 `FeaturesConfig` impl 块
- [x] `Some(names) if names.is_empty()` D-02 分支存在
- [x] 5 个单元测试全部通过（None/空/有序/单字段/全量反序）
- [x] `cargo clippy --all-targets -- -D warnings` 零警告（临时 `#[allow(dead_code)]`，Wave 2 接线后移除）
- [x] `cargo fmt` 已应用

## Notes

- `#[allow(dead_code)]` 临时加在 `ordered_field_indices()` 上，Wave 2 接线时移除
- Wave 2 的 CSV 和 SQLite exporter 均依赖此方法

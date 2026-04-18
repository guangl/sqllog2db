---
plan: "02-02"
status: complete
completed: "2026-04-18"
---

# Plan 02-02: CsvExporter ordered_indices 接线

## What Was Built

给 `CsvExporter` 添加 `ordered_indices: Vec<usize>` 字段，并将 `build_header()` 和
`write_record_preparsed()` 的投影路径改为按有序索引写出。

## Key Files

- `src/exporter/csv.rs` — 新增 `ordered_indices` 字段，重写 `build_header()`，投影路径改用 `match idx` 分发

## Self-Check: PASSED

- [x] `pub(crate) ordered_indices: Vec<usize>` 字段已添加
- [x] `build_header()` 改为 `for &idx in &self.ordered_indices` 遍历
- [x] 旧 `self.field_mask.is_active(i)` 逻辑已从 `build_header()` 移除
- [x] `write_record_preparsed()` 投影路径改为 `for &idx in ordered_indices { match idx { ... } }`
- [x] D-03：idx=14 在 normalize=false 时跳过
- [x] 全量掩码快速路径（FieldMask::ALL 分支）代码未动
- [x] 3 个 header 测试 + 2 个 field-order 测试全部通过
- [x] 全量测试套件绿，clippy 零警告

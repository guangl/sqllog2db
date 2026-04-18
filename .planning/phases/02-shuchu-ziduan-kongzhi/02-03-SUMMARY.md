---
plan: "02-03"
status: complete
completed: "2026-04-18"
---

# Plan 02-03: SqliteExporter ordered_indices 接线

## What Was Built

给 `SqliteExporter` 添加 `ordered_indices: Vec<usize>` 字段，并将 `build_create_sql()`、
`build_insert_sql()`、`do_insert_preparsed()` 改为按有序索引操作。

## Key Files

- `src/exporter/sqlite.rs` — 新增 `ordered_indices` 字段，重写 CREATE/INSERT SQL 构建，投影路径改用 `ordered_indices.iter().map(|&i| &all[i])`

## Self-Check: PASSED

- [x] `pub(super) ordered_indices: Vec<usize>` 字段已添加
- [x] `build_insert_sql(table_name, ordered_indices: &[usize])` 签名已更新
- [x] `build_create_sql(table_name, ordered_indices: &[usize])` 签名已更新
- [x] `initialize()` 调用点改为 `&self.ordered_indices`
- [x] `do_insert_preparsed()` 投影路径使用 `ordered_indices.iter().map(|&i| &all[i])`
- [x] 全量掩码快速路径（FieldMask::ALL + params![]）保留不变
- [x] 3 个 SQL builder 测试 + 1 个集成测试全部通过
- [x] 全量测试套件绿，clippy 零警告

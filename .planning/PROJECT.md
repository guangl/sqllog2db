# sqllog2db — SQL 过滤与字段投影增强

## Current Milestone: v1.0 增强 SQL 内容过滤与字段投影

**Goal:** 让用户能够精确指定"导出哪些记录的哪些字段"——过滤逻辑清晰可配置，输出结果完全可控

**Target features:**
- FILTER-01: 对任意字段支持正则表达式匹配过滤
- FILTER-02: 多关键词列表默认 AND 语义（全部满足才保留）
- FILTER-03: 支持排除模式（匹配则丢弃，而非匹配则保留）
- FIELD-01: 输出字段控制——用户可在 config.toml 中指定导出哪些字段

## What This Is

sqllog2db 是一个用于解析达梦数据库 SQL 日志文件并将其导出为 CSV 或 SQLite 的命令行工具。它以流式方式处理日志记录，通过可选的 Pipeline 过滤器处理后写入配置的导出器。本项目当前重点是增强内容过滤（支持正则、多条件、排除模式）和输出字段控制能力。

## Core Value

用户能够精确指定"导出哪些记录的哪些字段"——过滤逻辑清晰可配置，输出结果完全可控。

## Requirements

### Validated

- ✓ 流式解析达梦 SQL 日志文件 — existing
- ✓ 导出到 CSV 和 SQLite — existing
- ✓ Pipeline 过滤器（记录级 + 事务级） — existing
- ✓ 字段投影（FieldMask bitmask） — existing
- ✓ 参数归一化 / SQL 指纹 — existing
- ✓ 增量断点续传（resume state） — existing
- ✓ 并行 CSV 处理（rayon） — existing
- ✓ 基础 SQL 内容过滤 + 字段投影 — Phase 0 (committed)

### Active

- [ ] **FILTER-01**: 对任意字段支持正则表达式匹配过滤
- [ ] **FILTER-02**: 多关键词列表默认 AND 语义（全部满足才保留）
- [ ] **FILTER-03**: 支持排除模式（匹配则丢弃，而非匹配则保留）
- [ ] **FIELD-01**: 输出字段控制——用户可在 config 中指定导出哪些字段

### Out of Scope

- OR 条件组合 — 简单列表 AND 已满足需求，OR 增加配置复杂度
- 跨字段联合条件 — 暂不支持"字段A 满足 X 且 字段B 满足 Y"的复合谓词
- 运行时动态修改过滤规则 — 配置在启动时加载，不支持热重载

## Context

- 现有架构：`FiltersFeature` 在 `src/features/filters.rs`，两遍设计（pre-scan + main pass）
- `FieldMask` 已是 `u16` bitmask，15 个输出字段，exporters 已读取它
- 热循环中 `pipeline.is_empty()` 保证无过滤时零开销
- 当前 SQL 内容过滤仅对 `pm.sql`（DML 记录）做字符串包含检查
- 未提交改动：`run.rs`, `csv.rs`, `sqlite.rs`, `features/mod.rs`

## Constraints

- **性能**: 过滤逻辑不能破坏热循环的零开销快路径
- **配置格式**: 使用 TOML，与现有 `config.toml` 风格保持一致，列表默认 AND 语义
- **兼容性**: 不改变现有无过滤配置的行为

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| 列表默认 AND 语义 | 简单直观，覆盖最常见的"同时包含多个关键词"场景 | — Pending |
| 对任意字段过滤（非仅 sql_text） | 用户需求：按 user/schema/ip 等字段过滤 | — Pending |
| 正则通过 `regex` crate 实现 | Rust 生态标准选择，已在项目中使用 | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-17 after initialization*

# sqllog2db — 达梦 SQL 日志解析工具

## Current Milestone: v1.2 质量强化 & 性能深化

**Goal:** 消灭已知技术债，补全过滤缺口，进一步提升解析/过滤热路径与 CLI 启动速度。

**Target features:**
- ✓ [Tech Debt] sqlite.rs 静默错误修复 + table_name SQL 注入风险 — Phase 7 Complete (2026-05-10)
- [Tech Debt] Nyquist Phase 3/4/5/6 VALIDATION.md 补签
- [FILTER-03] 排除模式（匹配则丢弃）
- [PERF] 解析/过滤热路径进一步优化
- [PERF] CLI 启动 / 配置加载提速

## Shipped: v1.1 性能优化 ✅ (2026-05-10)

**Goal:** 通过 profiling 定位热点后，系统性提升 CSV 和 SQLite 导出性能，并降低内存/CPU 占用。

**Delivered (Phases 3–6):**
- ✓ Profile 热路径（flamegraph / criterion），定位 CSV 和 SQLite 实际瓶颈 — Phase 3
- ✓ CSV 写入吞吐优化（格式化、buffer、序列化）— Phase 4
- ✓ SQLite 写入速度优化（批量事务、prepared statement 复用；WAL 模式移除——数据无需崩溃保护）— Phase 5
- ✓ 利用 `dm-database-parser-sqllog` 1.0.0（mmap 零拷贝、par_iter、MADV_SEQUENTIAL 自动生效）— Phase 6
- ✓ 内存/CPU 占用优化（条件 reserve、热循环无分配）— Phase 4

## Shipped: v1.0 增强 SQL 内容过滤与字段投影 ✅ (2026-04-18)

## What This Is

sqllog2db 是一个用于解析达梦数据库 SQL 日志文件并将其导出为 CSV 或 SQLite 的命令行工具。它以流式方式处理日志记录，通过可选的 Pipeline 过滤器处理后写入配置的导出器。支持正则表达式多字段过滤（AND 语义）和输出字段精确控制。

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
- ✓ **FILTER-01**: 对任意字段支持正则表达式匹配过滤 — Phase 1
- ✓ **FILTER-02**: 多关键词列表默认 AND 语义（全部满足才保留） — Phase 1
- ✓ **FIELD-01**: 输出字段控制——用户可在 config 中指定导出哪些字段 — Phase 2

### Validated

- ✓ **PERF-01**: profile CSV 和 SQLite 热路径，生成 flamegraph/criterion 报告 — Phase 3
- ✓ **PERF-02**: CSV 写入吞吐优化（accept-defer：合成 -8.53%；上游解析层留 Phase 6）— Phase 4
- ✓ **PERF-03**: CSV 格式化路径优化（criterion micro-benchmark 验证）— Phase 4
- ✓ **PERF-08**: 热循环堆分配减少（条件 reserve + include_pm 兜底）— Phase 4

### Validated (v1.1)

- ✓ **PERF-04**: SQLite 批量事务 + prepared statement 复用 — Phase 5
- ✓ **PERF-05**: ~~SQLite WAL 模式~~ — 用户决策移除（数据无需崩溃保护）
- ✓ **PERF-06**: SQLite prepared statement 复用 — Phase 5
- ✓ **PERF-07**: dm-database-parser-sqllog 1.0.0 评估完成（index() 不集成，改进自动生效）— Phase 6
- ✓ **PERF-09**: 651 测试全部通过，0 失败 — Phase 6

### Active (v1.2)

- [ ] **DEBT-01**: sqlite.rs 静默错误修复——错误路径不再被忽略，记录到 error log
- [ ] **DEBT-02**: table_name SQL 注入风险——拼接 SQL 改为参数化或白名单校验
- [ ] **DEBT-03**: Nyquist Phase 3/4/5/6 VALIDATION.md 补签——补全缺失的 compliant 签署
- [ ] **FILTER-03**: 排除模式——配置中可指定"匹配则丢弃"规则，与现有包含过滤互补
- [ ] **PERF-10**: 解析/过滤热路径进一步优化——在 v1.1 profiling 基础上，识别并消除剩余瓶颈
- [ ] **PERF-11**: CLI 启动 / 配置加载提速——减少冷启动时间，提升 config 解析和 pipeline 初始化速度

### Out of Scope

- OR 条件组合 — 简单列表 AND 已满足需求，OR 增加配置复杂度
- 跨字段联合条件 — 暂不支持"字段A 满足 X 且 字段B 满足 Y"的复合谓词
- 运行时动态修改过滤规则 — 配置在启动时加载，不支持热重载

## Context

- 架构：`FiltersFeature` 在 `src/features/filters.rs`，两遍设计（pre-scan + main pass）
- `FilterProcessor` 热路径使用 `CompiledMetaFilters` + `CompiledSqlFilters`（预编译，启动时 validate）
- `ordered_indices: Vec<usize>` 注入到 `CsvExporter` / `SqliteExporter`，支持任意字段顺序投影
- 热循环中 `pipeline.is_empty()` 保证无过滤时零开销（未改动）
- Rust LOC: ~9,889 | 测试套件: 651 tests | 基准: ~5.2M records/sec (CSV synthetic)

## Constraints

- **性能**: 过滤逻辑不能破坏热循环的零开销快路径
- **配置格式**: 使用 TOML，与现有 `config.toml` 风格保持一致，列表默认 AND 语义
- **兼容性**: 不改变现有无过滤配置的行为

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| 列表默认 AND 语义 | 简单直观，覆盖最常见的"同时包含多个关键词"场景 | ✓ Phase 1 实现 |
| 对任意字段过滤（非仅 sql_text） | 用户需求：按 user/schema/ip 等字段过滤 | ✓ Phase 1 实现 |
| 正则通过 `regex` crate 实现 | Rust 生态标准选择，已在项目中使用 | ✓ Phase 1 实现 |
| ordered_indices Vec 替代 FieldMask 投影 | 支持任意字段顺序，FieldMask 只能全部/按默认顺序输出 | ✓ Phase 2 实现 |

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
*Last updated: 2026-05-10 — milestone v1.2 started*

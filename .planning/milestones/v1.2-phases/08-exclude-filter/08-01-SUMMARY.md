---
phase: 08-exclude-filter
plan: "01"
subsystem: features/filters
tags: [filter, exclude, regex, tdd]
dependency_graph:
  requires: []
  provides: [exclude-filter-core]
  affects: [src/features/filters.rs, src/cli/run.rs]
tech_stack:
  added: []
  patterns: [OR-veto, TDD-RED-GREEN, private-method-split]
key_files:
  modified:
    - src/features/filters.rs
    - src/cli/run.rs
decisions:
  - "exclude_veto() 和 include_and() 拆分为私有方法，使 should_keep() 主体保持 ≤ 40 行"
  - "validate_regexes() 拆分为 validate_include_regexes + validate_exclude_regexes，每个 ≤ 40 行"
  - "Task 1 commit 同时包含 should_keep OR-veto 重构，以消除 clippy dead-code 告警（exclude 字段需在 should_keep 中被使用）"
metrics:
  duration_minutes: 5
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
  tests_added: 21
  completed_date: "2026-05-10"
---

# Phase 8 Plan 01: 排除过滤器核心实现 Summary

**One-liner:** MetaFilters 新增 7 个 exclude_* 字段，OR-veto 语义在 should_keep() 中先于 include AND 检查执行，配合 validate_regexes 在启动阶段拦截非法 exclude 正则。

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | 扩展 MetaFilters/CompiledMetaFilters 支持 exclude 字段并重构 should_keep | e75ffae | src/features/filters.rs, src/cli/run.rs |
| 2 | 扩展 validate_regexes 校验 exclude 字段 + 新增 15 个 exclude 单元测试 | 10b9f08 | src/features/filters.rs |

## What Was Built

- `MetaFilters` struct 新增 7 个 `exclude_*: Option<Vec<String>>` 字段（D-01/D-02）
- `MetaFilters::has_filters()` 扩展：任一 exclude 字段非空即返回 true（D-06）
- `CompiledMetaFilters` struct 新增 7 个 `exclude_*: Option<Vec<Regex>>` 字段（D-03）
- `CompiledMetaFilters::from_meta()` 编译 7 个 exclude 字段（D-03）
- `CompiledMetaFilters::has_any_filters()` 新增方法，include 或 exclude 任一非空返回 true（D-05）
- `CompiledMetaFilters::should_keep()` 重构为调用 `exclude_veto() + include_and()`（D-04）
  - `exclude_veto()`: OR-veto 语义，任一 exclude 命中返回 true（应丢弃）
  - `include_and()`: 原有 AND 语义，完整保留
- `exclude_tags` 特殊处理：`meta.tag = None` 时不触发 exclude（保留记录）
- `FilterProcessor::new()` 改用 `has_any_filters()`，确保纯 exclude 配置激活 meta 路径（D-05）
- `FiltersFeature::validate_regexes()` 拆分为 `validate_include_regexes + validate_exclude_regexes`，每个 ≤ 40 行（D-08）

## Test Coverage

- 6 个 Task 1 RED 测试：结构体扩展和 has_any_filters 行为
- 15 个 Task 2 exclude 测试：OR-veto 语义、exclude+include 组合、exclude_tags 特殊处理、validate 非法正则
- 全量测试套件：301 passed, 0 failed

## Verification

```
grep -c "exclude_usernames" src/features/filters.rs  → 21 (≥5 ✓)
cargo test --lib features::filters::tests           → 66 passed ✓
cargo clippy --all-targets -- -D warnings           → 无输出 ✓
cargo fmt --check                                    → 无输出 ✓
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Task 1 clippy dead-code 需要 should_keep 同时实现**

- **Found during:** Task 1 commit（pre-commit hook 触发）
- **Issue:** 新增的 7 个 exclude_* 字段在 `CompiledMetaFilters` 中仅声明但未在 `should_keep()` 中使用，`has_any_filters()` 也未被调用；clippy `-D dead-code` 报错阻止提交
- **Fix:** 将 Task 2 的 `should_keep()` OR-veto 重构和 `run.rs` 中 `has_any_filters()` 调用合并进 Task 1 commit，Task 2 commit 仅包含 validate_regexes 扩展和 15 个新增测试
- **Files modified:** src/features/filters.rs, src/cli/run.rs
- **Commit:** e75ffae

**2. [Rule 2 - Missing Critical] validate_regexes 拆分为两个私有方法**

- **Found during:** Task 2 实现
- **Issue:** validate_regexes() 追加 7 个 exclude 校验后超过 40 行（CLAUDE.md 函数长度规范）
- **Fix:** 拆分为 `validate_include_regexes()` + `validate_exclude_regexes()` 两个私有方法，validate_regexes() 主体保持 ≤ 10 行
- **Files modified:** src/features/filters.rs
- **Commit:** 10b9f08

## Known Stubs

None — 所有 exclude 字段完整实现，无占位符或硬编码空值。

## Threat Surface Scan

无新增 trust boundary——exclude 字段与现有 include 字段共用相同的 TOML 配置路径和 validate_regexes 校验机制，威胁模型中 T-08-01 和 T-08-02 已覆盖。

## Self-Check: PASSED

- src/features/filters.rs: 存在 ✓
- src/cli/run.rs: 存在 ✓
- commit e75ffae: 存在 ✓
- commit 10b9f08: 存在 ✓

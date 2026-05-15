---
phase: 09-cli
plan: 01
subsystem: filtering
tags: [rust, regex, refactor, compile_patterns, filters]

requires:
  - phase: 08-exclude-filters
    provides: CompiledMetaFilters, CompiledSqlFilters 结构体及 exclude 字段

provides:
  - compile_patterns(field, patterns) 携带 field 名称返回 ConfigError::InvalidValue
  - CompiledMetaFilters::try_from_meta — 替代 from_meta，用 ? 传播错误
  - CompiledSqlFilters::try_from_sql_filters — 替代 from_sql_filters，用 ? 传播错误
  - 删除双重 regex 编译路径（validate 阶段和 compile 阶段各编译一次）

affects: [phase-10, cli/run.rs, config.rs]

tech-stack:
  added: []
  patterns:
    - "compile-is-validate: compile_patterns 承担验证职责，消除独立 validate 方法"
    - "try_new pattern: FilterProcessor::try_new 返回 Result，build_pipeline 传播错误"

key-files:
  created: []
  modified:
    - src/features/filters.rs
    - src/cli/run.rs
    - src/config.rs

key-decisions:
  - "FilterProcessor::new 改名为 try_new，build_pipeline 改为返回 Result<Pipeline>，使错误可以正确传播"
  - "config.rs validate() 改为直接调用 try_from_meta/try_from_sql_filters 完成验证，不再依赖已删除的 validate_regexes"

patterns-established:
  - "compile-is-validate: 不再有单独的 validate_pattern_list，compile_patterns 自带错误上报"

requirements-completed: [PERF-11]

duration: 15min
completed: 2026-05-14
---

# Phase 09 Plan 01: CLI Regex Compile Refactor Summary

**消除双重 regex 编译：compile_patterns 携带 field 名称直接返回 ConfigError，validate_regexes 系列方法彻底删除**

## Performance

- **Duration:** 15 min
- **Started:** 2026-05-14T00:00:00Z
- **Completed:** 2026-05-14T00:15:00Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments

- `compile_patterns` 新增 `field: &str` 参数，返回类型从 `Result<_, String>` 改为 `crate::error::Result<Option<Vec<Regex>>>`，错误消息包含字段路径和原始 pattern
- `CompiledMetaFilters::from_meta` → `try_from_meta`：14 个字段全部用 `?` 传播，删除所有 `.expect("regex validated")`
- `CompiledSqlFilters::from_sql_filters` → `try_from_sql_filters`：同样用 `?` 传播
- 删除 `validate_regexes`、`validate_include_regexes`、`validate_exclude_regexes`、`validate_pattern_list` 四个方法
- 715 个测试全部通过，cargo clippy --all-targets -- -D warnings 零 warning

## Task Commits

1. **Task 1: 重构 compile_patterns 签名并重命名 from_meta/from_sql_filters** - `1cf89cc` (refactor)

## Files Created/Modified

- `src/features/filters.rs` - compile_patterns 新签名、try_from_meta、try_from_sql_filters，删除 validate 系列，更新测试
- `src/cli/run.rs` - FilterProcessor::new → try_new（返回 Result），build_pipeline → 返回 Result<Pipeline>，调用处加 ?
- `src/config.rs` - validate() 改为调用 try_from_meta/try_from_sql_filters 完成验证

## Decisions Made

- `FilterProcessor::new` 改名为 `try_new` 返回 `Result<Self>`，并将 `build_pipeline` 从 `Pipeline` 改为 `Result<Pipeline>`，以正确传播 regex 编译错误。这是计划未明确说明的必要变更——计划只关注 filters.rs，但调用链需要同步更新。
- `config.rs` 的验证不再调用已删除的 `validate_regexes`，而是直接调用 `try_from_meta` + `try_from_sql_filters`，保持语义等价但避免重复编译。

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] FilterProcessor::new 返回类型更新**
- **Found during:** Task 1（验证阶段 cargo test 报错）
- **Issue:** 计划只描述修改 filters.rs，但 cli/run.rs 中 `FilterProcessor::new` 使用 `?` 操作符时返回 `Self` 导致编译错误
- **Fix:** 将 `new` 改名为 `try_new` 返回 `Result<Self>`；`build_pipeline` 改为返回 `Result<Pipeline>`；调用处添加 `?`
- **Files modified:** src/cli/run.rs
- **Verification:** cargo test 715 passed, cargo clippy 0 error
- **Committed in:** 1cf89cc（Task 1 提交）

---

**Total deviations:** 1 auto-fixed (Rule 1 Bug — 调用链未更新)
**Impact on plan:** 必要修复，无 scope creep。

## Issues Encountered

无其他问题。

## Self-Check

## Self-Check: PASSED

- `src/features/filters.rs` 存在 — FOUND
- `src/cli/run.rs` 存在 — FOUND
- `src/config.rs` 存在 — FOUND
- commit `1cf89cc` 存在 — FOUND（git rev-parse 已确认）

## Next Phase Readiness

- PERF-11 的双重编译根因已消除，regex 在 config validate 阶段编译一次，run 阶段直接使用已编译结果
- Phase 09 后续计划可在此基础上做进一步 CLI 启动性能优化

---
*Phase: 09-cli*
*Completed: 2026-05-14*

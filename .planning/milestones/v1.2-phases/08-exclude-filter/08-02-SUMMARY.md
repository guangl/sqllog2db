---
phase: 08-exclude-filter
plan: "02"
subsystem: cli/run + cli/init
tags: [filter, exclude, init-template, doc]
dependency_graph:
  requires: [exclude-filter-core]
  provides: [exclude-filter-integration]
  affects: [src/cli/run.rs, src/cli/init.rs]
tech_stack:
  added: []
  patterns: [OR-veto-routing, config-template-discovery]
key_files:
  modified:
    - src/cli/run.rs
    - src/cli/init.rs
decisions:
  - "exclude_* 注释示例按 include/exclude 紧邻配对排列，方便用户对比发现"
  - "ZH 模板同步将"模糊匹配"更正为"正则匹配"，消除文档与实际行为不一致"
  - "tags 字段在两个模板中均为新增（原模板缺失该字段注释）"
metrics:
  duration_minutes: 10
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
  tests_added: 0
  completed_date: "2026-05-11"
---

# Phase 8 Plan 02: 排除过滤器收尾工作 Summary

**One-liner:** FilterProcessor::new() 字段注释与 has_any_filters() 调用同步，init.rs 两个配置模板各补充 7 个 exclude_* 注释示例并将"模糊匹配"更正为"正则匹配"。

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | 更新 FilterProcessor::has_meta_filters 字段注释为 has_any_filters() | ddc09ba | src/cli/run.rs |
| 2 | 更新 init.rs 配置模板，插入 exclude_* 注释示例 | f9c2b4c | src/cli/init.rs |

## What Was Built

- `src/cli/run.rs`：`FilterProcessor::has_meta_filters` 字段文档注释从旧的 `has_filters()` 更新为 `has_any_filters()（include 或 exclude 任一）`，与 `FilterProcessor::new()` 中的实际调用一致（D-05）
- `src/cli/init.rs` ZH 模板（CONFIG_TEMPLATE_ZH）：
  - 7 个元数据字段（client_ips/usernames/sess_ids/thrd_ids/statements/appnames/tags）各插入 `exclude_*` 注释示例（D-09）
  - 将"支持模糊匹配"全部更正为"支持正则匹配"（6 个字段）
  - 新增原本缺失的 tags 和 exclude_tags 注释
- `src/cli/init.rs` EN 模板（CONFIG_TEMPLATE_EN）：
  - 7 个元数据字段各插入 `exclude_*` 注释示例（D-09）
  - 将"substring match"全部更正为"regex match"（6 个字段）
  - 新增原本缺失的 tags 和 exclude_tags 注释

## Verification

```
grep "has_any_filters" src/cli/run.rs           → 3 处（注释 × 2 + 调用 × 1）
grep -c "exclude_usernames" src/cli/init.rs     → 2（ZH + EN 各 1）
grep -c "exclude_" src/cli/init.rs              → 16（7 × 2 + exclude_patterns × 2）
grep "substring match" src/cli/init.rs          → 无输出（OK）
cargo test                                       → 323+342+50 passed, 0 failed ✓
cargo clippy --all-targets -- -D warnings       → 无 error/warning ✓
cargo fmt --check                               → 无输出 ✓
cargo run -- validate -c /tmp/test-exclude.toml → Configuration validation passed ✓
cargo run -- validate -c /tmp/test-invalid.toml → error: Invalid configuration value ... invalid regex ✓
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing] 计划期望 exclude_ 计数为 14，实际为 16**

- **Found during:** Task 2 验证
- **Issue:** 计划中的验收标准 `grep -c "exclude_" = 14` 未计入已存在的 `exclude_patterns`（SQL 级过滤器，ZH 和 EN 模板各 1 处 = 2 处），导致计数差异
- **Fix:** 确认实际内容正确（7 个 meta exclude 字段 × 2 模板 = 14 + 已有 exclude_patterns × 2 = 16），按实际正确计数
- **Files modified:** 无需修改（内容正确）

## Known Stubs

None — 所有 exclude_* 注释均为完整示例，无占位符。

## Threat Surface Scan

无新增 trust boundary。init.rs 模板内容为只读字符串常量，写入本地文件系统，不涉及外部输入（T-08-06 已 accept）。

## Self-Check: PASSED

- src/cli/run.rs：存在 ✓
- src/cli/init.rs：存在 ✓
- commit ddc09ba：存在 ✓
- commit f9c2b4c：存在 ✓

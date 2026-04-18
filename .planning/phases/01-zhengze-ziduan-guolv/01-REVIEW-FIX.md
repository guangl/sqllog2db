---
phase: 01-zhengze-ziduan-guolv
fixed_at: 2026-04-18T00:00:00Z
review_path: .planning/phases/01-zhengze-ziduan-guolv/01-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 01: Code Review Fix Report

**Fixed at:** 2026-04-18
**Source review:** `.planning/phases/01-zhengze-ziduan-guolv/01-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 3
- Fixed: 3
- Skipped: 0

## Fixed Issues

### WR-01: `merge_found_trxids` 跳过条件逻辑有歧义

**Files modified:** `src/features/filters.rs`
**Commit:** 7044ca8
**Applied fix:** 将 `merge_found_trxids` 中的跳过条件从 `(!self.enable && !self.has_filters()) || trxids.is_empty()` 简化为 `!self.enable || trxids.is_empty()`。原条件中 `&& !self.has_filters()` 是死代码——`has_filters()` 内部已经在 `!self.enable` 时短路返回 `false`，导致读者误以为存在两个独立判断，掩盖了真实意图且在未来修改 `has_filters()` 时容易静默失效。

### WR-02: `FiltersFeature::should_keep` 与热路径 OR/AND 语义分裂

**Files modified:** `src/features/filters.rs`, `src/cli/stats.rs`
**Commit:** 7044ca8
**Applied fix:** 为 `FiltersFeature::should_keep` 和 `MetaFilters::should_keep` 添加 `#[deprecated(note = "semantics differ from hot path; use CompiledMetaFilters::should_keep")]`，并附详细文档注释说明 OR/AND 语义差异。在调用方处理 deprecated 警告：`FiltersFeature::should_keep` 实现体加 `#[allow(deprecated)]`（合法自调用），`stats.rs` 的 `process_file` 函数加 `#[allow(deprecated)]` 并附注释说明 stats 场景下 OR 语义是预期行为，测试模块加模块级 `#[allow(deprecated)]`。所有位置均通过 `cargo clippy -- -D warnings`。

### WR-03: `sql.include_patterns`/`sql.exclude_patterns` 字段名与实现语义不符

**Files modified:** `src/features/filters.rs`
**Commit:** 7044ca8
**Applied fix:** 为 `SqlFilters` 结构体添加详细文档注释，明确说明字段使用字面量子串匹配（`str::contains`）而非正则表达式，警告用户不要在配置中填写正则语法（如 `^SELECT`、`\bDROP\b`）以防静默的语义错误，并指引用户使用 `record_sql` 字段（由 `CompiledSqlFilters` 处理，支持正则）。选择注释方案而非重命名字段，避免破坏现有配置文件兼容性。

---

_Fixed: 2026-04-18_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

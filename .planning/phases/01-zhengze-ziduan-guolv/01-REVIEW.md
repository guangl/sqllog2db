---
phase: 01-zhengze-ziduan-guolv
reviewed: 2026-04-18T00:00:00Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - Cargo.toml
  - src/cli/run.rs
  - src/config.rs
  - src/features/filters.rs
  - src/features/mod.rs
findings:
  critical: 0
  warning: 3
  info: 5
  total: 8
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-04-18
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

## Summary

本次审查覆盖了 Phase 1 新增的正则字段过滤功能：`CompiledMetaFilters`（跨字段 AND、字段内 OR）、`CompiledSqlFilters`（include/exclude 正则过滤）、`Config::validate()` 启动期正则校验，以及 `FilterProcessor` 的热路径重构。

整体设计合理，性能考量充分（预编译正则、`has_filters()` 快速路径、`parse_meta()` 共享复用）。发现以下问题：

- **3 个 Warning**：两处逻辑错误（条件歧义、OR/AND 语义分裂），一处缺失的正则校验
- **5 个 Info**：命名误导、死代码、函数过长等代码质量问题

无 Critical 级安全或崩溃问题。

---

## Warnings

### WR-01: `merge_found_trxids` 跳过条件逻辑有歧义

**File:** `src/features/filters.rs:172`

**Issue:**

```rust
if (!self.enable && !self.has_filters()) || trxids.is_empty() {
    return;
}
```

`has_filters()` 内部已经检查 `enable`，所以当 `enable=false` 时 `has_filters()` 恒为 `false`，`!self.enable && !self.has_filters()` 等价于 `!self.enable`。这让代码读者误以为存在两个独立条件，实则 `&& !self.has_filters()` 是死代码，掩盖了真实意图。若未来修改 `has_filters()` 不再短路 `enable`，此处就会静默失效。

**Fix:**

```rust
if !self.enable || trxids.is_empty() {
    return;
}
```

---

### WR-02: `FiltersFeature::should_keep` 与热路径 OR/AND 语义分裂

**File:** `src/features/filters.rs:148-168` 和 `src/features/filters.rs:319-353`

**Issue:**

`FiltersFeature::should_keep`（第148行）最终调用 `MetaFilters::should_keep`（第196行），该方法对各字段使用 **OR** 语义（任意一个字段命中即保留）。

热路径 `FilterProcessor::process_with_meta`（`run.rs:67`）调用的是 `CompiledMetaFilters::should_keep`（第319行），使用 **AND** 语义（所有字段都必须匹配才保留）。

这两个方法的语义相反，但结构相同——任何直接调用 `FiltersFeature::should_keep` 的代码（例如将来的单元测试或新功能）都会得到与实际导出行为不同的结果。`MetaFilters::should_keep` 是实际上已死的逻辑，但没有标记为废弃，存在被误用的风险。

**Fix:**

选择以下方案之一：

1. **标记废弃，防止误用：**

```rust
/// 已废弃：语义为 OR（任意字段命中即保留），与热路径 AND 语义不一致。
/// 热路径请使用 `CompiledMetaFilters::should_keep`。
#[deprecated(note = "semantics differ from hot path; use CompiledMetaFilters::should_keep")]
pub fn should_keep(&self, ts: &str, meta: &RecordMeta) -> bool { ... }
```

2. **删除 `FiltersFeature::should_keep` 和 `MetaFilters::should_keep`**，仅保留 `CompiledMetaFilters` 作为唯一过滤入口，在测试中直接构造 `CompiledMetaFilters`。

---

### WR-03: `validate_regexes` 未校验 `sql.include_patterns` / `sql.exclude_patterns`

**File:** `src/features/filters.rs:96-118`

**Issue:**

`validate_regexes` 仅校验了 `record_sql` 的两个 pattern 列表，遗漏了 `sql`（事务级预扫描过滤器）对应的两个列表：

```rust
// 当前：仅校验 record_sql
validate_pattern_list("features.filters.record_sql.include_patterns", ...)?;
validate_pattern_list("features.filters.record_sql.exclude_patterns", ...)?;
// 缺失：
// validate_pattern_list("features.filters.sql.include_patterns", ...)?;
// validate_pattern_list("features.filters.sql.exclude_patterns", ...)?;
```

注：`SqlFilters::matches` 目前用子串 `contains` 而非正则，所以 `sql` 字段的 pattern 实际不需要正则校验。但字段名叫 `*_patterns`，用户可能写入正则语法（`^SELECT`、`\bDROP\b` 等），启动期不会报错，运行时子串匹配会把 pattern 字符串当作字面值查找，导致静默的语义错误。

**Fix:**

根据实际设计意图选择：

- **如果 `sql` 字段未来要支持正则**：补充校验，并将 `SqlFilters::matches` 改为使用 `CompiledSqlFilters`。
- **如果 `sql` 字段只支持子串匹配**：将字段重命名为 `include_strings`/`exclude_strings`，并在注释中明确说明，防止用户误用正则语法。

---

## Info

### IN-01: `SqlFilters` 字段名 `include_patterns`/`exclude_patterns` 与实现语义不符

**File:** `src/features/filters.rs:88-92`, `455-483`

**Issue:** `SqlFilters.matches()` 用 `sql.contains(p)` 做子串匹配，但字段名为 `*_patterns`，TOML 配置文档会让用户误以为支持正则。

**Fix:** 重命名为 `include_substrings`/`exclude_substrings`，或在字段注释中明确写明"字面子串，不支持正则"。

---

### IN-02: `CompiledSqlFilters::has_filters` 被 `#[allow(dead_code)]` 压制

**File:** `src/features/filters.rs:382-385`

**Issue:** `#[allow(dead_code)]` 表明该方法没有调用点。如确实不需要，应删除；如确实需要保留（为将来使用），应加注释说明原因。

**Fix:** 删除该方法，或添加注释 `// 预留：供外部检查是否有编译后的 SQL 过滤条件`，并去掉 `#[allow(dead_code)]`（保留但接受 lint 警告，迫使后续使用者注意）。

---

### IN-03: `sql`（预扫描）与 `record_sql`（正式扫描）过滤语义不对称但 API 结构相同

**File:** `src/features/filters.rs:87-92`, `src/cli/run.rs:306-308`, `run.rs:651-658`

**Issue:** `sql` 和 `record_sql` 共用同一结构体 `SqlFilters`，但：
- `sql` → `SqlFilters::matches` → 子串匹配
- `record_sql` → `CompiledSqlFilters::matches` → 正则匹配

同一结构，两种行为，文档注释虽有说明，但调用方代码需要记住区分，容易引发维护错误。

**Fix:** 考虑为 `sql` 和 `record_sql` 使用不同的类型（`SubstringFilters` vs `RegexFilters`），在类型层面强制区分语义。

---

### IN-04: `process_log_file` 函数超过项目约定的 40 行上限

**File:** `src/cli/run.rs:108-270`

**Issue:** 函数约 162 行，违反 `CLAUDE.md` 中"Keep functions under 40 lines"的约定。函数承担了多个职责：进度条管理、记录过滤决策、参数替换调度、limit 检查、中断检测、导出写入。

**Fix:** 将以下逻辑提取为独立函数：
- 参数归一化决策（`compute_normalized` 调用路径）
- limit 检查与早退
- 进度条批量更新逻辑

---

### IN-05: `scan_log_file_for_matches` 内部重复了调用方已保证的过滤器存在检查

**File:** `src/cli/run.rs:285-288`

**Issue:** `handle_run` 已通过 `is_some_and(has_transaction_filters)` 保证只在有事务级过滤器时才调用 `scan_for_trxids_by_transaction_filters`，后者再调用 `scan_log_file_for_matches`。但 `scan_log_file_for_matches` 内部仍再次检查 `has_transaction_filters()`，属于冗余防御。

**Fix:** 添加注释说明此检查是防御性保留（"此函数可能被直接调用"），或将该函数改为只接受已验证存在的 `&FiltersFeature` 引用，将存在性检查提升到调用方。

---

_Reviewed: 2026-04-18_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

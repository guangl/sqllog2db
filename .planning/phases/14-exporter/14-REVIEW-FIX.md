---
phase: 14-exporter
fixed_at: 2026-05-16T00:00:00Z
review_path: .planning/phases/14-exporter/14-REVIEW.md
iteration: 1
findings_in_scope: 6
fixed: 6
skipped: 0
status: all_fixed
---

# Phase 14: Code Review Fix Report

**Fixed at:** 2026-05-16T00:00:00Z
**Source review:** .planning/phases/14-exporter/14-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 6
- Fixed: 6
- Skipped: 0

## Fixed Issues

### CR-01: guard negative f32 exectime before u64 cast in aggregation

**Files modified:** `src/cli/run.rs`
**Commit:** 135a800
**Applied fix:** Replaced bare `(pm.exectime * 1000.0) as u64` with a finite/positive guard expression. Added `u64::MAX as f32` upper-bound clamp to prevent silent data corruption for negative or non-finite exectime values. Added appropriate `#[allow(...)]` attributes (`cast_possible_truncation`, `cast_sign_loss`, `cast_precision_loss`) with an explanatory comment since the guard already ensures correctness.

### CR-02: wrap DDL inside transaction in write_template_stats

**Files modified:** `src/exporter/sqlite.rs`
**Commit:** dcf5f88
**Applied fix:** Moved the `DROP TABLE IF EXISTS` and `CREATE TABLE IF NOT EXISTS` DDL statements to execute after `BEGIN;` so they are part of the same transaction as the INSERT statements. Removed the now-unused `create_or_replace_template_table` helper method which was previously called before the transaction began.

### CR-03: quote first_seen and last_seen in companion CSV rows

**Files modified:** `src/exporter/csv.rs`
**Commit:** 5b6a4d3
**Applied fix:** Wrapped `first_seen` and `last_seen` fields in double-quotes using `write_csv_escaped`, consistent with how `template_key` is handled. Updated the companion CSV test assertions to match the new quoted format.

### WR-01: remove stale dead_code allows and planning comments

**Files modified:** `src/exporter/mod.rs`
**Commit:** ab53e91
**Applied fix:** Removed `#[allow(dead_code)]` from all three `write_template_stats` declarations (trait default on line 50, `ExporterKind` method on line 121, `ExporterManager::write_template_stats` on line 321). Removed the stale planning comment "Plan 04 将在 run.rs 接入此方法；骨架阶段暂未调用。" from both locations in mod.rs.

### WR-02: simplify parallel write_template_stats path

**Files modified:** `src/cli/run.rs`, `src/exporter/csv.rs`
**Commit:** 8bf266c
**Applied fix:** Used Option A from the review: changed `build_companion_path` and `write_companion_rows` from private `fn` to `pub(crate) fn` in csv.rs. Updated the parallel path in run.rs to call these functions directly instead of constructing an uninitialized `CsvExporter` + `ExporterManager` shell for dispatch.

### WR-03: add comment on defensive PARAMS guard in aggregation

**Files modified:** `src/cli/run.rs`
**Commit:** 7459042
**Applied fix:** Added a three-line comment above the `if record.tag.is_some()` guard explaining that it is a defensive check against PARAMS records (tag == None) that could reach the aggregation block via the `needs_pm=true` path under `do_normalize`, and that removing it would cause PARAMS records to be incorrectly counted in template statistics.

---

_Fixed: 2026-05-16T00:00:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

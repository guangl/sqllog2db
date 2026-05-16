---
phase: 12-sql
fixed_at: 2026-05-15T00:00:00Z
review_path: .planning/phases/12-sql/12-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 12: Code Review Fix Report

**Fixed at:** 2026-05-15T00:00:00Z
**Source review:** .planning/phases/12-sql/12-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 3
- Fixed: 3
- Skipped: 0

## Fixed Issues

### WR-01: `_tmpl_key` computed and immediately discarded — hot-loop does useless work

**Files modified:** `src/cli/run.rs`, `src/features/mod.rs`, `src/features/sql_fingerprint.rs`
**Commit:** 26b9137
**Applied fix:** Commented out the `_tmpl_key` assignment and `normalize_template` call in the
hot loop, replacing it with a Phase 13 placeholder comment. Renamed the `do_template` parameter
to `_do_template` in `process_log_file` to suppress the unused-variable warning. Added
`#[allow(dead_code)]` to `normalize_template`, `ScanMode::Normalize`, and the re-export in
`features/mod.rs` so they compile cleanly until Phase 13 wires in `TemplateAggregator::observe()`.

---

### WR-02: `handle_show_config` silently omits `[features.template_analysis]`

**Files modified:** `src/cli/show_config.rs`
**Commit:** ff9a51b
**Applied fix:** Added a rendering block for `template_analysis` after the existing `filters`
section, using the same `color::cyan` / `kv` / `println!()` pattern as the other feature
sections.

---

### WR-03: `Config::apply_one` has no arm for `features.template_analysis.enabled`

**Files modified:** `src/config.rs`
**Commit:** 2dd8018
**Applied fix:** Added a `"features.template_analysis.enabled"` match arm in `apply_one` using
`get_or_insert_with(Default::default)` and `parse_bool`, consistent with the existing
`features.filters.enable` and `features.replace_parameters.enable` arms.

---

_Fixed: 2026-05-15T00:00:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

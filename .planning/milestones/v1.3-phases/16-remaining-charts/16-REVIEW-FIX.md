---
phase: 16-remaining-charts
fixed_at: 2026-05-17T00:00:00Z
review_path: .planning/phases/16-remaining-charts/16-REVIEW.md
iteration: 2
findings_in_scope: 6
fixed: 6
skipped: 0
status: all_fixed
---

# Phase 16: Code Review Fix Report

**Fixed at:** 2026-05-17
**Source review:** .planning/phases/16-remaining-charts/16-REVIEW.md
**Iteration:** 2 (all findings fixed across both runs)

**Summary:**
- Findings in scope: 6 (1 Critical + 3 Warning + 2 Info)
- Fixed: 6
- Skipped: 0

## Fixed Issues

### CR-01: `apply_one` missing cases for `features.charts.trend_line` and `features.charts.user_pie`

**Files modified:** `src/config.rs`
**Commit:** (see git log for fix(16): CR-01)
**Applied fix:** Added two match arms immediately after the `latency_hist` arm in `Config::apply_one` for `"features.charts.trend_line"` and `"features.charts.user_pie"`, both parsing the value with `parse_bool` and updating the respective field via `get_or_insert_with(Default::default)`.

### WR-01: Pie sector outline draws filled black polygon instead of border

**Files modified:** `src/charts/user_pie.rs`
**Commit:** (see git log for fix(16): WR-01 WR-02)
**Applied fix:** Removed the second `root.draw(&Polygon::new(pts, BLACK.mix(0.3)))` call in the `render_pie` for-loop. Each sector is now drawn only once with `slice.color.filled()`, eliminating the semi-transparent black overlay that was making all pie slices visually muddy. Also removed the now-unnecessary `pts.clone()` since `pts` is no longer used twice.

### WR-02: Legend overflows SVG canvas when `top_n` >= 22

**Files modified:** `src/charts/user_pie.rs`
**Commit:** (see git log for fix(16): WR-01 WR-02)
**Applied fix:** Before creating the `SVGBackend` in `render_pie`, computed `required_h = 60 + (slices.len() as u32 + 1) * 25 + 20` and used `chart_h = required_h.max(600)` as the canvas height. Added `#[allow(clippy::cast_possible_truncation)]` for the `usize -> u32` cast (the slice count is bounded by `top_n` which is validated to be reasonable). The SVGBackend now uses `(1000, chart_h)` so the legend never overflows.

### WR-03: Integer overflow in y-axis upper bound

**Files modified:** `src/charts/trend_line.rs`
**Commit:** (see git log for fix(16): WR-03)
**Applied fix:** Changed `max_count * 11 / 10 + 1` to `max_count.saturating_mul(11) / 10 + 1` in `build_cartesian_2d`, preventing silent wrapping overflow in release builds and panics in debug builds when `max_count > u64::MAX / 11`.

### IN-01: Dead-code suppression `let _ = output_path` in `draw_chart`

**Files modified:** `src/charts/trend_line.rs`
**Commit:** (see git log for fix(16): IN-01)
**Applied fix:** Removed `output_path: &std::path::Path` from `draw_chart`'s parameter list,
removed the `let _ = output_path;` suppression line inside the function body, and updated
the call site in `draw_trend_line` to not pass `output_path`. Error conversion already happens
at the call site via `box_err_to_write_err`.

### IN-02: Duplicate `truncate_label` function in `frequency_bar.rs` and `user_pie.rs`

**Files modified:** `src/charts/mod.rs`, `src/charts/frequency_bar.rs`, `src/charts/user_pie.rs`
**Commit:** (see git log for fix(16): IN-02)
**Applied fix:** Extracted the shared Unicode-aware `truncate_label(key: &str, max_chars: usize) -> String`
function to `src/charts/mod.rs` as `pub(super)`. Removed the private copies from both
`frequency_bar.rs` and `user_pie.rs`. Updated call sites in production code to use
`super::truncate_label(...)`. Updated test modules in both files to explicitly import the
shared function via `use super::super::truncate_label;` since `use super::*;` no longer
brings it into scope.

---

_Fixed: 2026-05-17_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 2_

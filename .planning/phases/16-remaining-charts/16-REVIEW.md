---
phase: 16-remaining-charts
reviewed: 2026-05-17T00:00:00Z
depth: standard
files_reviewed: 7
files_reviewed_list:
  - src/charts/frequency_bar.rs
  - src/charts/mod.rs
  - src/charts/trend_line.rs
  - src/charts/user_pie.rs
  - src/cli/run.rs
  - src/features/mod.rs
  - src/features/template_aggregator.rs
findings:
  critical: 1
  warning: 3
  info: 2
  total: 6
status: issues_found
---

# Phase 16: Code Review Report

**Reviewed:** 2026-05-17
**Depth:** standard
**Files Reviewed:** 7
**Status:** issues_found

## Summary

Phase 16 extended `TemplateAggregator` with `hour_counts` and `user_counts` fields, added
`trend_line` and `user_pie` chart types to `ChartsConfig`, implemented both chart renderers,
and wired them into `generate_charts()`. The core data collection and aggregation logic is sound.
The primary blocker is a missing `apply_one` handler for the two new config keys; all other
findings are logic or visual bugs that affect correctness without crashing.

## Critical Issues

### CR-01: `apply_one` missing cases for `features.charts.trend_line` and `features.charts.user_pie`

**File:** `src/config.rs:357-364` (not in diff scope, but introduced by this phase's new fields)

**Issue:** `Config::apply_one` handles `"features.charts.frequency_bar"` and
`"features.charts.latency_hist"` (lines 351-362 of config.rs) but has no arms for
`"features.charts.trend_line"` or `"features.charts.user_pie"`. The `_ =>` fallthrough returns
`Err(unknown())`. Any user who writes:

```
--set features.charts.trend_line=false
```

gets a hard error at startup instead of a configuration override. This makes the new flags
unusable from the CLI override path, which is the standard way to disable individual charts
without editing the config file.

**Fix:** Add two arms to the `match` block in `apply_one` (config.rs, immediately after the
`latency_hist` arm):

```rust
"features.charts.trend_line" => {
    self.features
        .charts
        .get_or_insert_with(Default::default)
        .trend_line = parse_bool(value)?;
}
"features.charts.user_pie" => {
    self.features
        .charts
        .get_or_insert_with(Default::default)
        .user_pie = parse_bool(value)?;
}
```

## Warnings

### WR-01: Pie sector outline paints entire sector black at 30% opacity instead of drawing a border

**File:** `src/charts/user_pie.rs:167-169`

**Issue:** The code draws each sector twice — first with the slice color, then with
`BLACK.mix(0.3)`:

```rust
root.draw(&Polygon::new(pts.clone(), slice.color.filled()))
    .map_err(|e| to_write_err(output_path, &e))?;
root.draw(&Polygon::new(pts, BLACK.mix(0.3)))
    .map_err(|e| to_write_err(output_path, &e))?;
```

`BLACK.mix(0.3)` returns a `ShapeStyle` with `filled = true`, which fills the entire polygon
with 30% opacity black — it does not draw only a border/outline. The result is that all pie
slices are covered by a semi-transparent black wash, making colors visually muddy. The intent
appears to be a subtle sector border for contrast.

**Fix:** Use a stroke style for the outline, or simply remove the second draw call if no border
is desired:

```rust
// Option A: remove border entirely
root.draw(&Polygon::new(pts, slice.color.filled()))
    .map_err(|e| to_write_err(output_path, &e))?;

// Option B: draw a thin black stroke (if plotters supports it via PathElement / polyline)
```

### WR-02: Legend overflows SVG canvas when `top_n` is 22 or greater

**File:** `src/charts/user_pie.rs:112-134`

**Issue:** The legend is rendered at a fixed `legend_start_y = 60` with `row_h = 25` per entry.
The SVG canvas height is 600. With `top_n = N` slices plus an optional "Others" row, the legend
bottom reaches `60 + (N+1) * 25`. At `top_n = 22` that is `60 + 23*25 = 635`, which is 35 px
below the canvas boundary. Any user who sets `top_n` above 21 gets a silently clipped or
overflowing SVG legend.

The `ChartsConfig` default for `top_n` is 10, so this is not triggered by the defaults. However
the config validation only rejects `top_n == 0`; there is no upper bound check.

**Fix:** Either clamp the displayed legend entries independently of `top_n`, or dynamically
increase the canvas height based on slice count:

```rust
// In render_pie, compute required height before creating the SVGBackend:
let required_h = 60 + (slices.len() as u32 + 1) * 25 + 20;
let chart_h = required_h.max(600);
let root = SVGBackend::new(output_path, (1000, chart_h)).into_drawing_area();
```

### WR-03: Integer overflow in `draw_chart` y-axis upper bound when `max_count` is close to `u64::MAX`

**File:** `src/charts/trend_line.rs:74`

**Issue:**

```rust
.build_cartesian_2d((0..n).into_segmented(), 0u64..(max_count * 11 / 10 + 1))?;
```

`max_count * 11` overflows `u64` if `max_count > u64::MAX / 11` (≈ 1.68 × 10^18). In a
production environment processing a log file with billions of SQL statements, this is
theoretically reachable if many statements fall in the same one-hour bucket. The overflow causes
the chart to render with an incorrect (wrapped) y-axis upper bound and the series to appear at
the wrong scale. In release mode Rust wraps silently; in debug mode it panics.

**Fix:**

```rust
let y_max = max_count.saturating_mul(11) / 10 + 1;
.build_cartesian_2d((0..n).into_segmented(), 0u64..y_max)?;
```

## Info

### IN-01: Dead-code suppression `let _ = output_path` in `draw_chart`

**File:** `src/charts/trend_line.rs:109`

**Issue:** `output_path` is passed as a parameter to `draw_chart` solely to enable error
conversion in `draw_trend_line`, but the inner function never uses it. The `let _ = output_path`
line silences the compiler warning but indicates that the parameter is extraneous.

**Fix:** Remove `output_path` from `draw_chart`'s parameter list; do the error conversion at
the call site in `draw_trend_line` (as is already done with `box_err_to_write_err`). The
function signature already passes the path to `map_err` on the outside.

```rust
fn draw_chart<DB: DrawingBackend>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    labels: &[String],
    counts: &[u64],
    max_count: u64,
    n: usize,
    // remove output_path
) -> Result<(), Box<dyn std::error::Error + 'static>>
```

### IN-02: Duplicate `truncate_label` function across `frequency_bar.rs` and `user_pie.rs`

**File:** `src/charts/frequency_bar.rs:94`, `src/charts/user_pie.rs:12`

**Issue:** Both files contain an identical private `truncate_label(name: &str, max_chars: usize) -> String`
implementation. The logic is non-trivial (Unicode-aware, with ellipsis). Any future bug fix in
one copy must be manually applied to the other.

**Fix:** Extract to a shared `src/charts/label.rs` (or inline into `src/charts/mod.rs`) and
re-export it for use by both renderers:

```rust
// src/charts/label.rs
pub(super) fn truncate_label(key: &str, max_chars: usize) -> String { ... }
```

---

_Reviewed: 2026-05-17_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

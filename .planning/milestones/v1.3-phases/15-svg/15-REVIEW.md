---
phase: "15"
reviewed: 2026-05-16T00:00:00Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - src/cli/show_config.rs
  - src/config.rs
  - src/features/mod.rs
  - src/features/template_aggregator.rs
findings:
  critical: 2
  warning: 2
  info: 1
  total: 5
status: issues_found
---

# Phase 15: Code Review Report

**Reviewed:** 2026-05-16T00:00:00Z
**Depth:** standard
**Files Reviewed:** 4
**Status:** issues_found

## Summary

Phase 15 added `ChartsConfig` (struct + serde/default/apply_one), a charts dependency check in `validate()` / `validate_and_compile()`, `ChartEntry` + `iter_chart_entries()` in `TemplateAggregator`, and a re-export of `ChartEntry` in `features/mod.rs`.

The histogram logic and merge semantics in `template_aggregator.rs` are sound. The charts dependency gate (requiring `template_analysis.enabled = true`) is correctly duplicated across both validation paths and is well-tested. However, two BLOCKER-level gaps exist: `show_config` silently drops the entire `[features.charts]` section, and `output_dir` is never validated for emptiness — the only required field in `ChartsConfig` that has no guard.

---

## Critical Issues

### CR-01: `handle_show_config` does not render `[features.charts]`

**File:** `src/cli/show_config.rs:122-127`
**Issue:** The function renders every config section that exists (`[sqllog]`, `[logging]`, `[exporter.csv]`, `[exporter.sqlite]`, `[features.replace_parameters]`, `[features.filters]`, `[features.template_analysis]`) but has no branch for `cfg.features.charts`. A user running `show-config` against a config that includes `[features.charts]` will see no output for that section, giving the misleading impression the charts config is absent or inactive. The omission also means `--diff` mode never highlights a changed `output_dir` or `top_n`.

**Fix:**
```rust
// Append after the template_analysis block (after line 126):
if let Some(charts) = &cfg.features.charts {
    println!("{}", color::cyan("[features.charts]"));
    kv("output_dir", &charts.output_dir, None, diff);
    kv("top_n", &charts.top_n.to_string(), None, diff);
    kv(
        "frequency_bar",
        &charts.frequency_bar.to_string(),
        None,
        diff,
    );
    kv(
        "latency_hist",
        &charts.latency_hist.to_string(),
        None,
        diff,
    );
    println!();
}
```

---

### CR-02: `ChartsConfig.output_dir` is never validated for emptiness

**File:** `src/config.rs:80-93` (and mirrored at lines 143-156)
**Issue:** `output_dir` is declared as `pub output_dir: String` with no `#[serde(default)]` — meaning the field is required in TOML — but once parsed, its value is never checked for emptiness or whitespace-only strings. Every analogous path-like field in the codebase has an explicit guard:
- `logging.file` → `LoggingConfig::validate()` line 389
- `exporter.csv.file` → `CsvExporter::validate()` line 477
- `exporter.sqlite.database_url` → `SqliteExporter::validate()` line 522
- `sqllog.path` → `SqllogConfig::validate()` line 346

The charts dependency check in `validate()` / `validate_and_compile()` only gates on whether `template_analysis.enabled` is true; it never inspects the content of `output_dir`. An empty or whitespace-only `output_dir` passes all validation and will cause a runtime I/O error when chart generation is actually implemented.

Additionally, `apply_one("features.charts.output_dir", "")` succeeds silently.

**Fix:**
```rust
// In Config::validate() (and validate_and_compile()), after the ta_enabled check:
if self.features.charts.is_some() {
    let ta_enabled = self
        .features
        .template_analysis
        .as_ref()
        .is_some_and(|ta| ta.enabled);
    if !ta_enabled {
        return Err(/* existing error */);
    }
    // Add:
    let charts = self.features.charts.as_ref().unwrap();
    if charts.output_dir.trim().is_empty() {
        return Err(Error::Config(ConfigError::InvalidValue {
            field: "features.charts.output_dir".to_string(),
            value: charts.output_dir.clone(),
            reason: "charts output_dir cannot be empty".to_string(),
        }));
    }
}
```

---

## Warnings

### WR-01: `ChartsConfig.top_n = 0` is not rejected by validation

**File:** `src/config.rs:296-308`
**Issue:** `top_n: 0` is semantically invalid (generate a chart of the top 0 templates), yet no validation rejects it. The analogous field `exporter.sqlite.batch_size` has an explicit `batch_size == 0` guard (line 552). `apply_one("features.charts.top_n", "0")` succeeds silently and produces a stored `top_n = 0` that will be passed to chart generation logic.

**Fix:**
```rust
// In Config::validate() / validate_and_compile(), inside the charts block:
if charts.top_n == 0 {
    return Err(Error::Config(ConfigError::InvalidValue {
        field: "features.charts.top_n".to_string(),
        value: "0".to_string(),
        reason: "top_n must be greater than 0".to_string(),
    }));
}
```

---

### WR-02: Silently discarded `histogram.add()` error in `merge()`

**File:** `src/features/template_aggregator.rs:93`
**Issue:** `let _ = entry.histogram.add(&other_entry.histogram)` discards the `Result`. `hdrhistogram::Histogram::add` returns `Err` when the source histogram's value range exceeds the target's bounds. Both histograms are created with `new_with_bounds(1, 60_000_000, 2)` today, so the bounds are always compatible — but this invariant is not enforced by the type system. A future change that creates a `TemplateEntry` with different bounds (e.g., a different `sigfig` or max value) would cause silent data loss in `merge()` with no indication in logs or test failures.

**Fix:**
```rust
// Replace the silent discard with an expect() that documents the invariant:
entry
    .histogram
    .add(&other_entry.histogram)
    .expect("histogram bounds mismatch: all TemplateEntry histograms must use identical bounds");
```

---

## Info

### IN-01: `#[allow(unused_imports)]` is the wrong lint suppressor for a public re-export

**File:** `src/features/mod.rs:12-13`
**Issue:** The annotation reads:
```rust
#[allow(unused_imports)] // Phase 15 Plan 03+ 将实现图表生成时使用
pub use template_aggregator::ChartEntry;
```
`unused_imports` suppresses the warning for an imported name that is never used *within the same file*. However, `pub use` re-exports the symbol for downstream crates/modules; `unused_imports` is not the lint that fires for unused public exports. The correct lint would be `dead_code` if the item were private. Since `ChartEntry` is `pub`, neither lint actually fires in practice — but the attribute and comment together mislead future readers about what is being suppressed and why. The attribute can be removed entirely; the comment clarifying future use is sufficient.

**Fix:** Remove the `#[allow(unused_imports)]` attribute. Keep the comment if desired.

---

_Reviewed: 2026-05-16T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

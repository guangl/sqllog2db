---
phase: 09-cli
reviewed: 2026-05-14T00:00:00Z
depth: standard
files_reviewed: 16
files_reviewed_list:
  - benches/BENCHMARKS.md
  - benches/bench_csv.rs
  - benches/bench_filters.rs
  - benches/bench_sqlite.rs
  - src/cli/init.rs
  - src/cli/preflight.rs
  - src/cli/run.rs
  - src/cli/show_config.rs
  - src/cli/update.rs
  - src/config.rs
  - src/exporter/csv.rs
  - src/exporter/mod.rs
  - src/exporter/sqlite.rs
  - src/features/filters.rs
  - src/main.rs
  - tests/integration.rs
findings:
  critical: 0
  warning: 0
  info: 2
  total: 2
status: fixed
---

# Phase 09: Code Review Report

**Reviewed:** 2026-05-14
**Depth:** standard
**Files Reviewed:** 16
**Status:** fixed

## Summary

The codebase is well-structured with good error handling conventions and a clear data flow model. Test coverage is thorough. Two critical defects were found: a logic correctness bug that causes transaction-level filters to silently produce wrong output when combined with the precompiled-filters optimization (SC-2), and a partial data-loss risk in the parallel CSV concat path when a file operation fails mid-loop. Three warnings cover the SQLite exec_time column type mismatch, missing `apply_overrides` support for `features.*` keys, and a TOCTOU race in the preflight output-writable check.

---

## Critical Issues

### CR-01: Pre-compiled `CompiledMetaFilters` does not include trxids discovered during transaction pre-scan

**File:** `src/cli/run.rs:621-667`

**Issue:** `validate_and_compile()` is called in `main.rs` before `handle_run` and compiles `CompiledMetaFilters` from the original config. When transaction-level filters (`indicators` or `sql`) are present, `handle_run` performs a pre-scan (lines 649-665) that discovers matching trxids and calls `merge_found_trxids` to populate `final_cfg.features.filters.meta.trxids`. However, `build_pipeline` is called on line 667 with the *original* `compiled_meta` that was compiled before the pre-scan, so `CompiledMetaFilters.trxids` is empty/stale. The `FilterProcessor` in the main pass therefore never matches against the pre-scan-discovered trxids, meaning the entire purpose of the transaction pre-scan—retaining records belonging to matching transactions—is silently defeated. Records that should be kept are dropped.

The bug only manifests when both conditions hold simultaneously:
1. `features.filters.indicators` or `features.filters.sql` is configured (triggers pre-scan).
2. The caller passes non-`None` `compiled_filters` (the `run` command path in `main.rs`).

The integration test `test_handle_run_with_transaction_filters_prescans` at line 925 passes `compiled_filters: None`, so the bug is not exercised by the existing test suite.

**Fix:** After `merge_found_trxids`, re-compile `CompiledMetaFilters` from `final_cfg` rather than reusing the stale pre-scan `compiled_meta`:

```rust
// In handle_run, replace the current pre-scan block + build_pipeline call with:
let (compiled_meta_final, compiled_sql) = match compiled_filters {
    Some((m, s)) => (Some(m), Some(s)),
    None => (None, None),
};

let final_cfg: &Config = if cfg
    .features
    .filters
    .as_ref()
    .is_some_and(crate::features::FiltersFeature::has_transaction_filters)
{
    let extra_trxids = scan_for_trxids_by_transaction_filters(&log_files, cfg, jobs);
    let mut tmp = cfg.clone();
    if let Some(f) = &mut tmp.features.filters {
        f.merge_found_trxids(extra_trxids.into_iter().collect());
    }
    owned_cfg = tmp;
    &owned_cfg
} else {
    cfg
};

// Re-derive compiled_meta from final_cfg so merged trxids are included
let compiled_meta_for_pipeline = if final_cfg.features.filters
    .as_ref()
    .is_some_and(|f| f.enable)
{
    final_cfg.features.filters.as_ref().map(|f| {
        crate::features::CompiledMetaFilters::try_from_meta(&f.meta)
    }).transpose()?
} else {
    None
};

let pipeline = build_pipeline(final_cfg, compiled_meta_for_pipeline);
```

---

### CR-02: Partial output file left in corrupted state when `concat_csv_parts` fails mid-loop

**File:** `src/cli/run.rs:416-435, 588-596`

**Issue:** In `concat_csv_parts`, `std::io::copy` (line 429) writes part data into the output file, then `std::fs::remove_file(part_path)?` removes the temp file (line 430). If either operation returns an error for a middle part, the function propagates the error. The caller at lines 594-596 then removes the temp directory but does **not** remove the partially written output file. The output file now contains data from parts `0..idx-1` only—a silently corrupted/truncated CSV. Resume state is not updated (the interrupt check at line 727 prevents it), but the corrupt output file persists on disk with no indication of truncation.

**Fix:** When `concat_csv_parts` returns an error, remove the partial output to prevent silent data corruption:

```rust
let concat_result = concat_csv_parts(
    &parts_for_concat,
    output_path,
    csv_cfg.overwrite,
    append_to_existing,
);
let _ = std::fs::remove_dir_all(&parts_dir);
if concat_result.is_err() && !append_to_existing {
    // Output file is partially written — remove it to prevent corrupt data
    let _ = std::fs::remove_file(output_path);
}
concat_result?;
```

---

## Warnings

### WR-01: SQLite `exec_time_ms` column stores raw `f32` milliseconds as `REAL`, diverging from CSV's `i64` representation

**File:** `src/exporter/sqlite.rs:102, 208`

**Issue:** The `exec_time_ms` column is declared `REAL` (line 102) and stored as `Value::Real(f64::from(v))` where `v` is `f32` milliseconds (line 208). The CSV exporter writes this field via `f32_ms_to_i64` which truncates to integer milliseconds (`i64`). Users querying across both exporters see different types and potentially different values for the same logical field. The `f32_ms_to_i64` helper in `exporter/mod.rs` was introduced specifically for this conversion; its non-use in the SQLite path appears unintentional.

**Fix:**

```rust
// src/exporter/sqlite.rs line ~208 — in do_insert_preparsed:
exec_time.map_or(Value::Null, |v| Value::Integer(super::f32_ms_to_i64(v))),
```

Update `COL_TYPES[11]` from `"REAL"` to `"INTEGER"` accordingly.

---

### WR-02: `apply_overrides` / `apply_one` silently rejects all `features.*` config keys

**File:** `src/config.rs:169-244`

**Issue:** The `apply_one` match covers `sqllog.*`, `logging.*`, and `exporter.*` keys but has no arm for `features.*`. Any `--set features.filters.enable=true` or `--set features.replace_parameters.enable=false` hits the `_ => return Err(unknown())` branch and fails with "unknown config key". The `--set` flag is the primary mechanism for CI/scripting overrides and its documented dot-path notation implies `features.*` keys are supported. This gap forces users to modify the TOML file for what could be a one-off override.

**Fix:** Add cases for the most useful keys:

```rust
"features.filters.enable" => {
    self.features
        .filters
        .get_or_insert_with(Default::default)
        .enable = parse_bool(value)?;
}
"features.replace_parameters.enable" => {
    self.features
        .replace_parameters
        .get_or_insert_with(Default::default)
        .enable = parse_bool(value)?;
}
```

---

### WR-03: TOCTOU race between existence check and open in `check_path_writable`

**File:** `src/cli/preflight.rs:56-60`

**Issue:** `check_path_writable` reads `path.exists()` and then conditionally calls `OpenOptions::new().append(true).open(path)`. Between these two syscalls, the file could be deleted or made inaccessible (network share, concurrent process), producing a false-positive "writable" result. Additionally, the preflight open uses `append(true)` while the actual exporter opens with `write(true).truncate(...)`, so the two operations test different conditions—a file could be appendable but not truncatable (e.g., append-only flag set on Linux with `chattr +a`).

**Fix:** Consolidate into a single open that mirrors actual exporter behavior:

```rust
fn check_path_writable(file_path: &str, result: &mut PreflightResult) {
    let path = Path::new(file_path);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        if !parent.exists() {
            if std::fs::create_dir_all(parent).is_err() {
                result.errors.push(format!("无法创建输出目录: {}", parent.display()));
            }
            return;
        }
    }
    // Mirror the actual exporter open flags
    if std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(path)
        .is_err()
    {
        result.errors.push(format!("输出文件不可写: {file_path}"));
    }
}
```

---

## Info

### IN-01: Background update-check thread may log after logger is torn down

**File:** `src/cli/update.rs:67-93`

**Issue:** `check_for_updates_at_startup` spawns a thread and drops the `JoinHandle` immediately (fire-and-forget). If the main thread completes and the process starts tearing down, the background thread may call `warn!` (lines 83-86) after the logger backend has been deinitialized. On some logger backends (e.g., file-backed loggers) this can panic or produce a garbled entry. The comment at line 92 acknowledges the fire-and-forget pattern but does not address the logger lifetime concern.

**Suggestion:** Return the `JoinHandle` to the caller and join it with a timeout before exiting, or gate the log calls behind an `AtomicBool` shutdown flag.

---

### IN-02: Redundant `path.exists()` check in `handle_init` success message

**File:** `src/cli/init.rs:49-53`

**Issue:** After `fs::write(path, content)` succeeds (line 42), `path.exists()` on line 49 will always be `true`. The condition `force && path.exists()` is therefore equivalent to just `force`. The code is not incorrect (the log messages are accurate), but the redundant call is misleading—it reads as if there is a scenario where the write succeeded but the file does not exist.

**Suggestion:**

```rust
if force {
    info!("Configuration file overwritten: {output_path}");
} else {
    info!("Configuration file generated: {output_path}");
}
```

---

_Reviewed: 2026-05-14_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

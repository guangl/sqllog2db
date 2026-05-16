---
phase: 14-exporter
reviewed: 2026-05-16T00:00:00Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - src/exporter/mod.rs
  - src/features/mod.rs
  - src/exporter/sqlite.rs
  - src/exporter/csv.rs
  - src/cli/run.rs
findings:
  critical: 3
  warning: 3
  info: 2
  total: 8
status: issues_found
---

# Phase 14: Code Review Report

**Reviewed:** 2026-05-16T00:00:00Z
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

## Summary

Phase 14 adds `write_template_stats()` to the `Exporter` trait and wires it into both the sequential and parallel `handle_run` paths. The implementations in `CsvExporter` and `SqliteExporter` are structurally sound, but three correctness defects exist: a silent data-loss bug from negative-float to `u64` truncation in aggregation, a transaction isolation error in the SQLite `write_template_stats` path when called after `finalize()`, and unquoted `first_seen`/`last_seen` values in companion CSV rows that will corrupt any consumer that contains commas or quotes in those strings. Two additional quality warnings cover stale `#[allow(dead_code)]` attributes that were never removed after wiring the calls into `run.rs`, and a logic asymmetry in the parallel path that builds a dummy `CsvExporter` + `ExporterManager` shell purely to dispatch through the enum (unnecessary indirection). Two informational items cover a function length violation and a misleading comment.

---

## Critical Issues

### CR-01: Negative `f32` `exectime` silently wraps to large `u64` in aggregator

**File:** `src/cli/run.rs:241`
**Issue:** `pm.exectime` is `f32`. When the parser produces a negative exectime (e.g. due to clock skew or corrupt log data), `(pm.exectime * 1000.0) as u64` is a saturating/wrapping cast in Rust: a negative `f32` cast to `u64` yields `0` on most platforms under the "C-style" semantics codified since Rust 1.45, but this relies on the LLVM `fptoui` sanitized path. Any value less than `0.0` becomes `0`. If the float is negative and very large in magnitude (e.g. `f32::NEG_INFINITY`), behavior prior to 1.45 was UB; even in 1.45+, the result (`0`) silently discards the actual exectime, which then gets clamped to `1` inside `TemplateEntry` and injected into the histogram as `1 us`. This produces subtly wrong percentile/average data with no warning, no error, and no way to detect the corruption post-hoc.

The companion `f32_ms_to_i64()` helper (already present in `src/exporter/mod.rs:376`) handles exactly this — `is_finite()`, clamp, truncate — but the aggregation path bypasses it entirely and open-codes its own cast.

**Fix:**
```rust
// src/cli/run.rs:241 — replace
let exectime_us = (pm.exectime * 1000.0) as u64;

// with a saturating, finite-checked conversion
let exectime_us = if pm.exectime.is_finite() && pm.exectime > 0.0 {
    (pm.exectime * 1000.0).min(u64::MAX as f32) as u64
} else {
    0
};
```
Or extract a small helper analogous to `f32_ms_to_i64` and share it.

---

### CR-02: SQLite `write_template_stats` opens a new transaction on a connection that may be in autocommit, but `create_or_replace_template_table` executes DDL *outside* any transaction

**File:** `src/exporter/sqlite.rs:446-468`
**Issue:** `write_template_stats` calls `self.create_or_replace_template_table()` first (line 446), then issues `BEGIN;` (line 451), then inserts rows, then `COMMIT;`. The DDL (`DROP TABLE IF EXISTS` and `CREATE TABLE IF NOT EXISTS`) runs *before* the `BEGIN;`. In SQLite, DDL executed outside an explicit transaction is auto-committed immediately. If any subsequent step fails (e.g., an `INSERT` on a duplicate `template_key` due to a bug, or a disk-full error mid-batch), the `sql_templates` table has already been dropped-and-recreated with no rows, and there is no way to roll back the DDL. The correct order is: `BEGIN` → DDL → DML → `COMMIT`.

Additionally, the `BEGIN;` on line 451 will fail with `"cannot start a transaction within a transaction"` if called while the main export transaction is still open (i.e., when `write_template_stats` is called *before* `finalize()`). The test suite works around this by always calling `finalize()` first, but the call ordering is not enforced by the type system. In the sequential path in `run.rs` line 900, `exporter_manager.finalize()` is called on line 891, so the main transaction is committed; however the `conn` field is still `Some(...)`, so the method does not fail the `is_initialized` check. This is fragile — if the call order is ever changed, a runtime error results.

**Fix:** Reorder `write_template_stats` to wrap the DDL inside the transaction:
```rust
fn write_template_stats(&mut self, stats: &[crate::features::TemplateStats], _final_path: Option<&std::path::Path>) -> Result<()> {
    let conn = self.conn.as_ref().ok_or_else(|| Self::db_err("write_template_stats: not initialized"))?;
    conn.execute_batch("BEGIN;").map_err(|e| Self::db_err(format!("begin failed: {e}")))?;
    // DDL inside transaction — SQLite supports transactional DDL
    if self.overwrite {
        conn.execute("DROP TABLE IF EXISTS sql_templates", []).map_err(|e| Self::db_err(format!("drop sql_templates failed: {e}")))?;
    }
    conn.execute("CREATE TABLE IF NOT EXISTS sql_templates (template_key TEXT NOT NULL PRIMARY KEY, count INTEGER NOT NULL, avg_us INTEGER NOT NULL, min_us INTEGER NOT NULL, max_us INTEGER NOT NULL, p50_us INTEGER NOT NULL, p95_us INTEGER NOT NULL, p99_us INTEGER NOT NULL, first_seen TEXT NOT NULL, last_seen TEXT NOT NULL)", []).map_err(|e| Self::db_err(format!("create sql_templates failed: {e}")))?;
    // ... insert rows ...
    conn.execute_batch("COMMIT;").map_err(|e| Self::db_err(format!("commit failed: {e}")))?;
    Ok(())
}
```

---

### CR-03: `first_seen` and `last_seen` written unquoted in companion CSV — injection / corruption for any value containing a comma

**File:** `src/exporter/csv.rs:55-57`
**Issue:** `format_companion_row` writes `first_seen` and `last_seen` as raw bytes with no CSV quoting or escaping. The `TemplateStats` struct stores these as `String`, populated from `record.ts.as_ref()` (line 242 of `run.rs`). The DaMeng timestamp format used by the log parser (`"2025-01-15 10:30:28.001"`) never contains commas, so existing tests pass. However:

1. The field is `pub` and `String` — there is nothing preventing a caller (or future code path, e.g. a config-injected override) from storing a timestamp that includes a comma or double-quote.
2. More importantly, if a future change makes `first_seen`/`last_seen` carry a timezone suffix like `"2025-01-15 10:30:28.001+08:00"`, the value is safe, but if it ever carries locale-dependent formats with commas (`"January 15, 2025"`) the CSV is silently corrupted.
3. The inconsistency is a latent defect: `template_key` is double-quoted and escaped; numeric fields need no quoting; `first_seen`/`last_seen` are in a third category (strings that happen not to need quoting today).

The correct fix is to quote and escape all string columns consistently:
```rust
// src/exporter/csv.rs — format_companion_row, lines 54-57
buf.push(b'"');
write_csv_escaped(buf, s.first_seen.as_bytes());
buf.push(b'"');
buf.push(b',');
buf.push(b'"');
write_csv_escaped(buf, s.last_seen.as_bytes());
buf.push(b'"');
buf.push(b'\n');
```

---

## Warnings

### WR-01: Stale `#[allow(dead_code)]` on three items that are now actively called

**File:** `src/exporter/mod.rs:50`, `src/exporter/mod.rs:121`, `src/exporter/mod.rs:321`
**Issue:** All three `write_template_stats` declarations in `mod.rs` — the trait default (line 50), the `ExporterKind` method (line 121), and `ExporterManager::write_template_stats` (line 321) — carry `#[allow(dead_code)]`. These were added during the skeleton phase ("Plan 04 will wire this") and were never removed. Phase 14 has now wired the calls: `run.rs` calls `exporter_manager.write_template_stats(stats, None)` (line 900) and `tmp_em.write_template_stats(stats, Some(...))` (line 798). The `dead_code` allows are now factually wrong and will suppress any future legitimate Clippy `dead_code` warnings on these items if the wiring is ever broken again. The comment on line 49 ("骨架阶段暂未调用") is also stale.

**Fix:** Remove all three `#[allow(dead_code)]` attributes and update or delete the comment on line 49.

---

### WR-02: Parallel `write_template_stats` path constructs an uninitialized `CsvExporter` shell as dispatch vehicle

**File:** `src/cli/run.rs:796-798`
**Issue:** In the parallel path, after `process_csv_parallel` returns the merged aggregator, the code creates a brand-new `CsvExporter` (line 796) and wraps it in an `ExporterManager` (line 797) purely to call `write_template_stats`. This exporter is never `initialize()`d. The `write_template_stats` dispatch for `ExporterKind::Csv` goes directly to `CsvExporter::write_template_stats`, which in turn calls `build_companion_path` and `write_companion_rows` — neither of which touches `self.writer`. So at present, the code works because the CsvExporter implementation happens not to use `self.writer` in this method. However:

1. It creates a misleading object that appears uninitialized (e.g., it would fail any call to `export()`).
2. The entire wrapper is unnecessary — `CsvExporter::write_template_stats` is a pure function of `stats` and `final_path`; it can be called as a free function or directly:

```rust
// src/cli/run.rs:795-799 — replace with direct call
if let Some(csv_cfg) = final_cfg.exporter.csv.as_ref() {
    crate::exporter::csv::write_companion_rows_pub(
        &build_companion_path(Path::new(&csv_cfg.file)),
        stats,
    )?;
}
```
Alternatively, make `write_companion_rows` a `pub(crate)` free function and call it directly. If the current approach is kept, at minimum add a comment explaining why `initialize()` is intentionally not called.

---

### WR-03: `exectime_us` aggregation skips PARAMS records but does not guard against records with `tag == None` bypassing the `record.tag.is_some()` check at a different nesting level

**File:** `src/cli/run.rs:233-242`
**Issue:** The aggregation block reads:
```rust
if let Some(ref mut agg) = aggregator {
    if record.tag.is_some() {
        let tmpl_key = crate::features::normalize_template(pm.sql.as_ref());
        ...
        agg.observe(&tmpl_key, exectime_us, record.ts.as_ref());
    }
}
```
This is nested inside the `if passes { ... }` block (line 186), which is itself inside `let needs_pm = passes || (do_normalize && record.tag.is_none()); if needs_pm { ... }` (line 181). The path `needs_pm=true, passes=false` is only taken when `do_normalize=true && record.tag.is_none()` (PARAMS record that failed the pipeline filter). In that case `aggregator` access is correctly skipped because `passes=false`. The logic is correct but fragile: the guard `record.tag.is_some()` inside the aggregation block re-checks a condition that was already implied by `passes` at an outer level, creating two different guards for the same semantic constraint with no comment explaining the inner guard's necessity. If someone refactors the outer structure, the inner guard may be accidentally removed, causing PARAMS records to be counted in aggregation.

**Fix:** Add a comment at line 233 explaining that the inner `record.tag.is_some()` check is defensive and explains what PARAMS records are in this context; or restructure so the inner guard is provably redundant via type/control-flow rather than convention.

---

## Info

### IN-01: `process_log_file` exceeds the 40-line function limit

**File:** `src/cli/run.rs:116-307`
**Issue:** `process_log_file` is approximately 191 lines (lines 116–307). The project enforces a ≤40-line function rule per `CLAUDE.md`. This function has grown substantially over successive phases.

**Fix:** Extract sub-units: the inner record processing block (lines 164-283) is a candidate for a named helper such as `process_record`, accepting the relevant local state by parameter or a small context struct.

---

### IN-02: Stale planning comment in production code

**File:** `src/exporter/mod.rs:49`, `src/exporter/mod.rs:320`
**Issue:** Line 49: `// Plan 04 将在 run.rs 接入此方法；骨架阶段暂未调用。` and line 320: `// Plan 04 将在 run.rs 接入此方法；骨架阶段暂未调用。` are planning-phase comments that remain in shipped code. Phase 14 has completed wiring; these comments now describe a state that no longer exists.

**Fix:** Remove both planning-phase comments. If documentation is needed, replace with a description of the method's contract.

---

_Reviewed: 2026-05-16T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

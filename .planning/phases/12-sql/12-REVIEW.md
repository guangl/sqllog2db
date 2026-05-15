---
phase: 12-sql
reviewed: 2026-05-15T00:00:00Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - src/cli/init.rs
  - src/cli/run.rs
  - src/cli/show_config.rs
  - src/features/mod.rs
  - src/features/sql_fingerprint.rs
findings:
  critical: 0
  warning: 3
  info: 2
  total: 5
status: issues_found
---

# Phase 12: Code Review Report

**Reviewed:** 2026-05-15T00:00:00Z
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

## Summary

Phase 12 adds SQL template normalization via a new `normalize_template()` function backed by a shared `scan_sql_bytes()` scan engine (refactored from the existing `fingerprint()` path), a new `TemplateAnalysisConfig` struct wired into `FeaturesConfig`, and hot-loop integration via a `do_template` flag.

The core logic in `sql_fingerprint.rs` is well-structured: comment stripping, IN-list folding, keyword uppercasing, and whitespace collapsing all work correctly. The `ScanMode` refactor is clean and the existing `fingerprint()` tests pass with no regression.

Three issues rise to WARNING level:

1. The `_tmpl_key` result computed in the hot loop is immediately discarded — the feature produces CPU and allocation work with zero observable effect, and the config flag `template_analysis.enabled = true` silently does nothing useful today.
2. `handle_show_config` does not display `[features.template_analysis]`, creating a blind spot when diagnosing configs with the `show-config` subcommand.
3. `Config::apply_one` has no arm for `features.template_analysis.enabled`, so `--set features.template_analysis.enabled=true` silently returns an "unknown key" error at runtime.

---

## Warnings

### WR-01: `_tmpl_key` computed and immediately discarded — hot-loop does useless work

**File:** `src/cli/run.rs:223-227`

**Issue:** When `do_template == true`, `normalize_template(pm.sql.as_ref())` is called on every exported record, allocating a `String` and then assigning it to `_tmpl_key`. The value is never read, passed anywhere, or aggregated. The comment says "供 Phase 13 TemplateAggregator::observe() 消费", which confirms this is an intentional stub, but the cost is real right now: every call allocates heap memory (capacity ≈ sql length), runs the full scan engine, and immediately frees the result. A user who enables `template_analysis.enabled = true` gets silent CPU overhead and no output.

The correct approach for a stub is either to skip calling `normalize_template` entirely (guard the whole block with a `#[allow(unused)]` comment and a `todo!()` placeholder so it's clear it's unfinished), or to leave `do_template` always `false` until Phase 13 is ready to consume the value.

**Fix:** Guard the computation so it is not reached until Phase 13 ships, or remove the call entirely and leave a commented-out placeholder:

```rust
// D-14: Phase 13 will wire this into TemplateAggregator::observe().
// let _tmpl_key = if do_template {
//     Some(crate::features::normalize_template(pm.sql.as_ref()))
// } else {
//     None
// };
```

Alternatively, keep the code but gate it behind a compile-time feature flag so it compiles but is never reachable in release builds until Phase 13.

---

### WR-02: `handle_show_config` silently omits `[features.template_analysis]`

**File:** `src/cli/show_config.rs:86-120`

**Issue:** The `handle_show_config` function renders `[features.replace_parameters]` and `[features.filters]` sections but has no code for `[features.template_analysis]`. Running `sqllog2db show-config -c config.toml` on a config with `[features.template_analysis] enabled = true` produces no output for that section — the user cannot tell whether the feature is active.

This is the same pattern applied to the two pre-existing features; the new feature should follow it.

**Fix:** Add the rendering block after the `replace_parameters` section:

```rust
if let Some(ta) = &cfg.features.template_analysis {
    println!("{}", color::cyan("[features.template_analysis]"));
    kv("enabled", &ta.enabled.to_string(), None, diff);
    println!();
}
```

---

### WR-03: `Config::apply_one` has no arm for `features.template_analysis.enabled`

**File:** `src/config.rs:243-257` (context) — missing arm for the new key

**Issue:** `Config::apply_one` handles `"features.filters.enable"` and `"features.replace_parameters.enable"` but has no arm for `"features.template_analysis.enabled"`. A user passing `--set features.template_analysis.enabled=true` on the CLI receives an `Error::Config(ConfigError::InvalidValue { reason: "unknown config key ..." })` error instead of the expected override taking effect. This is an inconsistency with the other features and breaks the `--set` contract for the new config key.

Note: the config key uses `enabled` (not `enable`) to match the struct field name; the arm must use that exact key.

**Fix:** Add an arm in `apply_one`:

```rust
"features.template_analysis.enabled" => {
    self.features
        .template_analysis
        .get_or_insert_with(Default::default)
        .enabled = parse_bool(value)?;
}
```

---

## Info

### IN-01: `is_ident_byte` includes `.` — handle_word consumes qualified identifiers as single tokens

**File:** `src/features/sql_fingerprint.rs:316-318`

**Issue:** `is_ident_byte` returns `true` for `b'.'`, so `handle_word` scans across the dot and treats `schema.table` or `t.column` as a single token. This is intentional (qualified names are preserved as-is). However it also means a word like `outer_schema.FROM` would be scanned as one token, preventing the `FROM` part from being uppercased. This is actually the desired behavior (qualified names should not have their components uppercased), but it is not documented with an example. A brief comment noting the dot-inclusive ident rule and why it is intentional would reduce future confusion.

**Fix:** Add a one-line comment:

```rust
/// Single byte is an identifier byte (letter, digit, underscore, or dot).
/// Including `.` ensures qualified names like `schema.table` are treated
/// as a single token and not split into keyword candidates.
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'.'
}
```

---

### IN-02: `NEEDS_SPECIAL_NORM` adds `-` and `/` as special bytes, increasing dispatch overhead on `fingerprint()` path

**File:** `src/features/sql_fingerprint.rs:3-20`

**Issue:** The old lookup table (`NEEDS_SPECIAL`) did not mark `-` (0x2D) or `/` (0x2F) as special. The new shared table `NEEDS_SPECIAL_NORM` adds both so the normalize path can detect `--` and `/*` comment starts. In `fingerprint` (Fingerprint) mode, both bytes now break the bulk-copy inner loop and fall through to `dispatch_byte`'s default arm (`out.push(b); i + 1`), which is functionally identical to the old bulk copy but one byte at a time. SQL with many arithmetic expressions (e.g. `price / qty`, `end_date - start_date`) will see a modest throughput regression on the fingerprint path compared to the old single-table design.

This is a quality/performance tradeoff note, not a correctness bug. For v1 scope it is informational only.

**Fix (optional):** If fingerprint throughput becomes a concern, restore a separate `NEEDS_SPECIAL_FP` table that omits `-` and `/`, and select the table at the top of `scan_sql_bytes` based on `mode`. This avoids changing any other logic.

---

_Reviewed: 2026-05-15T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

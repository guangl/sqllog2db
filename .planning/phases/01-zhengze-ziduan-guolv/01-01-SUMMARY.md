---
phase: 01-zhengze-ziduan-guolv
plan: "01"
subsystem: features/filters
tags: [regex, filtering, compilation, validation, tdd]
requirements: [FILTER-01, FILTER-02]

dependency_graph:
  requires: []
  provides:
    - CompiledMetaFilters (with AND cross-field semantics)
    - CompiledSqlFilters (with regex include/exclude)
    - compile_patterns helper
    - match_any_regex helper
    - FiltersFeature::validate_regexes
    - Config::validate regex gate
  affects:
    - src/features/filters.rs
    - src/config.rs
    - Cargo.toml

tech_stack:
  added:
    - regex = "1" (Vec<Regex> with linear-time NFA, no ReDoS risk)
  patterns:
    - TDD RED/GREEN with cargo test
    - Pre-compile at startup, expect() in hot path (validated upstream)
    - AND semantics across fields via early-return pattern
    - Intra-field OR semantics via Iterator::any()

key_files:
  created: []
  modified:
    - Cargo.toml (add regex dependency)
    - src/features/filters.rs (add CompiledMetaFilters, CompiledSqlFilters, helpers, validate_regexes, tests)
    - src/config.rs (add filters.validate_regexes() call in Config::validate, add tests)

decisions:
  - "Use Vec<Regex> instead of RegexSet: supports short-circuit any() for small lists (1-5 patterns)"
  - "Compile at startup via from_meta/from_sql_filters, expect() is safe post-validate()"
  - "mark new items #[allow(dead_code)] until Plan 02 wires them into FilterProcessor hot path"
  - "None/empty patterns -> Ok(None) in compile_patterns (not configured = pass-through)"

metrics:
  duration: "~20 minutes"
  completed: "2026-04-18"
  tasks_completed: 1
  tasks_total: 1
  files_modified: 7
---

# Phase 01 Plan 01: Regex Filter Core — Summary

**One-liner:** Pre-compiled regex filter structs (`CompiledMetaFilters` + `CompiledSqlFilters`) with AND cross-field / OR intra-field semantics, startup validation via `Config::validate()`, and full TDD test coverage using the `regex` crate.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 (RED) | Add failing tests for compiled regex structures | a3fdb17 | Cargo.toml, src/features/filters.rs |
| 1 (GREEN) | Implement CompiledMetaFilters, CompiledSqlFilters, helpers, validation | 587b910 | src/features/filters.rs, src/config.rs, Cargo.lock, fmt files |

## What Was Built

### Core Logic (`src/features/filters.rs`)

- **`compile_patterns(Option<&[String]>) -> Result<Option<Vec<Regex>>, String>`** — compiles regex strings; `None`/empty returns `Ok(None)` (not configured = pass-through)
- **`match_any_regex(Option<&[Regex]>, &str) -> bool`** — `None` = `true`, `Some([])` = `true`, otherwise `any(re.is_match)`
- **`validate_pattern_list(field, patterns)`** — validates at startup, returns `ConfigError::InvalidValue` on failure
- **`CompiledMetaFilters`** — pre-compiled regex for all 7 meta fields + `TrxidSet`; `should_keep()` uses AND cross-field, OR intra-field semantics (D-04, D-02)
- **`CompiledSqlFilters`** — pre-compiled include/exclude regex for `record_sql`; `matches()` applies include then exclude logic (D-03)
- **`FiltersFeature::validate_regexes()`** — validates all 9 pattern lists, called from `Config::validate()`

### Validation Gate (`src/config.rs`)

Added to `Config::validate()`:
```rust
if let Some(filters) = &self.features.filters {
    if filters.enable {
        filters.validate_regexes()?;
    }
}
```

### Tests Added

**`src/features/filters.rs` (14 new tests):**
- `test_compile_patterns_none/empty/valid/invalid`
- `test_match_any_regex_none_passes/empty_passes/match/no_match`
- `test_compiled_meta_unconfigured_passes`
- `test_compiled_meta_and_semantics` — both usernames AND client_ips must match
- `test_compiled_meta_single_field_or` — any of 2 username patterns passes
- `test_compiled_meta_tags_none_rejected` — tag=None with configured tag patterns fails
- `test_compiled_meta_trxids_and` — trxids participates in AND gate
- `test_compiled_sql_include_regex` / `test_compiled_sql_exclude_regex`

**`src/config.rs` (2 new tests):**
- `test_validate_invalid_regex_in_filters` — `[invalid` triggers ConfigError with field name
- `test_validate_valid_regex_in_filters` — `^admin.*` passes validation

## TDD Gate Compliance

- RED gate: commit `a3fdb17` — `test(01-01): add failing tests...` (tests fail to compile)
- GREEN gate: commit `587b910` — `feat(01-01): implement...` (all 50 tests pass)
- REFACTOR gate: Not needed (clippy fixes folded into GREEN commit)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Clippy: redundant guards in match arms**
- **Found during:** GREEN verification
- **Issue:** `Some(v) if v.is_empty()` in `compile_patterns` and `match_any_regex` are redundant guards
- **Fix:** Replaced with `None | Some([]) => ...` pattern
- **Files modified:** src/features/filters.rs

**2. [Rule 1 - Bug] Clippy: missing `#[must_use]` on `from_meta` and `from_sql_filters`**
- **Found during:** GREEN verification
- **Fix:** Added `#[must_use]` attribute
- **Files modified:** src/features/filters.rs

**3. [Rule 1 - Bug] Clippy: missing `# Panics` doc sections on methods that call `.expect()`**
- **Found during:** GREEN verification
- **Fix:** Added `# Panics` sections to `from_meta` and `from_sql_filters`
- **Files modified:** src/features/filters.rs

**4. [Rule 1 - Bug] Clippy: `map_or(true, ...)` should use `is_none_or`**
- **Found during:** GREEN verification
- **Fix:** Replaced `.map_or(true, |p| ...)` with `.is_none_or(|p| ...)`
- **Files modified:** src/features/filters.rs

**5. [Rule 2 - Missing] Dead code warnings for future-use public items**
- **Found during:** GREEN verification
- **Issue:** `CompiledMetaFilters`, `CompiledSqlFilters`, helper functions not yet wired to `FilterProcessor` (Plan 02 work)
- **Fix:** Added `#[allow(dead_code)]` to all new items pending Plan 02 wiring
- **Files modified:** src/features/filters.rs

**6. [Rule 1 - Bug] `cargo fmt` formatting differences in existing files**
- **Found during:** GREEN verification
- **Fix:** Ran `cargo fmt`; affected `run.rs`, `csv.rs`, `sqlite.rs`, `mod.rs`, `config.rs`, `filters.rs`
- **Note:** These were pre-existing formatting issues surfaced by fmt run, not caused by this plan

## Known Stubs

None. All new structures are fully implemented and tested. The `#[allow(dead_code)]` items are complete implementations awaiting Plan 02 integration, not stubs.

## Threat Flags

No new network endpoints, auth paths, file access patterns, or schema changes introduced.

T-01-03 mitigation verified: `ConfigError::InvalidValue { field, value, reason }` includes field name and pattern value but no sensitive data — confirmed in test `test_validate_invalid_regex_in_filters`.

## Self-Check

### Created Files

None — all changes are modifications to existing files.

### Modified Files

- FOUND: src/features/filters.rs
- FOUND: src/config.rs
- FOUND: Cargo.toml

### Commits

- FOUND: a3fdb17 (RED gate)
- FOUND: 587b910 (GREEN gate)

## Self-Check: PASSED

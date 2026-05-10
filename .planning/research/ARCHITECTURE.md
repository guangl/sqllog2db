# Architecture Research

**Domain:** Rust CLI — streaming log parser with pipeline filtering and multi-format export
**Researched:** 2026-05-10
**Confidence:** HIGH (all findings from direct source code inspection)

## Standard Architecture

### System Overview

```
CLI entry (main.rs)
    ↓ Config::from_file() + apply_overrides() + validate()
    ↓ logging::init_logging()
    ↓ cli::run::handle_run()
         ↓
    [Optional pre-scan phase]
    scan_for_trxids_by_transaction_filters()  ← rayon par_iter across files
         ↓ merge_found_trxids() → FiltersFeature.meta.trxids
         ↓
    build_pipeline()
         └→ FilterProcessor::new(&FiltersFeature)
               ├─ CompiledMetaFilters::from_meta()  ← regex compilation
               └─ CompiledSqlFilters::from_sql_filters()  ← separate, record-level

    [Main streaming pass]
    LogParser::from_path()
    parser.iter()  (or par_iter() for parallel CSV)
         ↓ each record
    pipeline.is_empty()?
    ├─ YES → fast path, no parse_meta()
    └─ NO  → record.parse_meta() → pipeline.run_with_meta()
                  └→ FilterProcessor::process_with_meta()
                        ├─ time range (string compare, no regex)
                        ├─ CompiledMetaFilters::should_keep()  ← AND semantics
    [SQL record-level filter — applied separately outside pipeline]
    sql_record_filter.matches(pm.sql)
         ↓ passes
    ExporterManager::export_one_preparsed()
         └→ CsvExporter or SqliteExporter
```

### Component Responsibilities

| Component | Responsibility | Key Detail |
|-----------|----------------|------------|
| `FiltersFeature` (config) | Deserialization, validation dispatch, pre-scan detection | Has deprecated `should_keep()` with OR semantics; hot path uses compiled structs |
| `CompiledMetaFilters` | Pre-compiled regex filters, AND semantics across fields | Built once at startup from `MetaFilters`; `has_filters` bool pre-cached |
| `CompiledSqlFilters` | Pre-compiled regex for `record_sql` include/exclude | Built in `handle_run()`, passed separately — NOT in pipeline |
| `FilterProcessor` | `LogProcessor` impl, wraps `CompiledMetaFilters` + time range | Lives inside `Pipeline`; reuses caller's `MetaParts` to avoid re-parse |
| `Pipeline` | Ordered list of `LogProcessor` impls; `is_empty()` fast path | `run_with_meta()` short-circuits on first false |
| `SqlFilters` | Literal substring match for transaction-level pre-scan `sql` field | NOT regex — field name misleading, code comment warns about this |
| `ExporterManager` | Enum dispatch to CsvExporter/SqliteExporter/DryRunExporter | Eliminates vtable overhead on hot path |
| `SqliteExporter` | Parameterized INSERT via `prepare_cached()`; batch COMMIT | SQL built dynamically in `initialize()` from `ordered_indices` |

## Current Data Flow for Filters

### Startup Sequence (relevant to PERF-11)

```
main.rs: run()
  1. clap arg parsing (lang detection + CommandFactory + from_arg_matches)
  2. color::init()
  3. check_for_updates_at_startup()          ← network I/O at startup
  4. load_config()                            ← fs::read_to_string + toml::from_str
  5. cfg.apply_overrides()
  6. cfg.validate()
       └→ filters.validate_regexes()          ← Regex::new() per pattern (validates only)
  7. logging::init_logging()
  8. preflight::check()
  9. handle_run():
       a. SqllogParser::new().log_files()     ← directory scan / glob
       b. [optional] scan_for_trxids...()    ← pre-scan, regex built inside
       c. build_pipeline()
            └→ FilterProcessor::new()
                 └→ CompiledMetaFilters::from_meta()  ← Regex::new() AGAIN (compiles)
                 └→ has_meta_filters bool
       d. CompiledSqlFilters::from_sql_filters()       ← Regex::new() AGAIN
```

**PERF-11 observation:** Regex patterns are validated (compiled) once in `validate_regexes()`,
then compiled AGAIN in `CompiledMetaFilters::from_meta()` and `CompiledSqlFilters::from_sql_filters()`.
This is two full compilations per pattern. The compiled `Regex` objects from validation are discarded.

### Hot Loop (process_log_file)

```
for result in parser.iter():
  if pipeline.is_empty():
    passes = true   // zero overhead — no parse_meta()
  else:
    meta = record.parse_meta()   // shared with FilterProcessor
    ok = pipeline.run_with_meta(&record, &meta)
    // FilterProcessor reuses meta, no second parse_meta()

  if passes:
    pm = record.parse_performance_metrics()  // skipped if !include_pm (D-05/D-06)
    if sql_record_filter.matches(pm.sql):    // applied OUTSIDE pipeline
      normalized = compute_normalized(...)
      exporter_manager.export_one_preparsed(...)
```

## FILTER-03: Exclusion Mode Integration

### Current State

`SqlFilters` (transaction-level, literal match) already has `exclude_patterns` — implemented and working.

`CompiledSqlFilters` (`record_sql`, regex match) also has `exclude_patterns` — implemented and working.

`CompiledMetaFilters` (metadata fields: username, ip, sess, thrd, stmt, app, tag, trxid) has **no exclusion** concept. All fields are include-only.

### What FILTER-03 Means

FILTER-03 is about adding exclusion patterns for **metadata fields** — i.e., "discard records where username matches X" or "discard records where ip matches Y". The SQL-level exclusion already exists.

### Integration Design

**Option A (recommended): Add exclude fields to `MetaFilters` config + `CompiledMetaFilters`**

Add parallel `exclude_*` fields alongside existing include fields in `MetaFilters`:

```toml
[features.filters]
enable = true
usernames = ["^admin"]          # include: keep if any match
exclude_usernames = ["^sys"]    # exclude: discard if any match
```

In `CompiledMetaFilters`, add corresponding `exclude_*: Option<Vec<Regex>>` fields.

In `should_keep()`, evaluate exclusions after inclusions. Short-circuit order:
1. Time range (no struct needed)
2. Exclusion check — if ANY exclude pattern matches ANY field → discard immediately
3. Inclusion check — all configured include fields must match

**Exclusion before inclusion** is the correct order: exclusion is a hard veto, more likely to be a small set of patterns.

```rust
// In CompiledMetaFilters::should_keep():
// Step 1: Exclusion (discard immediately if matches)
if self.exclude_usernames.as_deref().is_some_and(|p| p.iter().any(|re| re.is_match(meta.user))) {
    return false;
}
// ... other exclude_* fields ...

// Step 2: Inclusion (existing AND logic)
if !match_any_regex(self.usernames.as_deref(), meta.user) { return false; }
// ...
```

**Option B: Separate ExcludeProcessor in pipeline**

Add a second `Box<dyn LogProcessor>` after `FilterProcessor`. Cleaner separation but adds pipeline overhead (second pass through `processors.iter().all()`, second `process_with_meta()` dispatch). The fast path (`pipeline.is_empty()`) is already lost once any filter is configured, so marginal overhead is small. However, it doubles config surface and complicates the mental model.

**Recommendation: Option A.** Exclusion is semantically part of the same filter evaluation; users expect `exclude_usernames` to live beside `usernames`, not as a separate feature block.

### Config Surface (new fields)

```toml
[features.filters]
enable = true
# existing include fields unchanged
usernames = ["^admin"]

# new exclude_* fields (FILTER-03)
exclude_usernames = ["^sys_"]
exclude_client_ips = ["^10\\.0\\."]
exclude_sess_ids = []
exclude_thrd_ids = []
exclude_statements = []
exclude_appnames = []
exclude_tags = []
# exclude_trxids uses exact match (no regex), consistent with trxids
exclude_trxids = ["99999"]
```

### Files to Modify for FILTER-03

| File | Change | Type |
|------|--------|------|
| `src/features/filters.rs` | Add `exclude_*` fields to `MetaFilters`; add `exclude_*` `Option<Vec<Regex>>` to `CompiledMetaFilters`; update `from_meta()`, `should_keep()`, `has_filters()` | **MODIFY** |
| `src/features/filters.rs` | Add `validate_regexes()` entries for `exclude_*` fields | **MODIFY** |
| `src/features/mod.rs` | No changes needed | — |
| `src/cli/run.rs` | No changes needed (exclusion runs inside `FilterProcessor`) | — |

## PERF-10: Hot Path Optimization

### Current Hot Path Analysis

From source inspection:

1. **`parse_meta()` sharing** — already done. Pipeline reuses caller's `MetaParts`, no double parse.
2. **`parse_performance_metrics()` skipping** — already done via `include_pm` flag (D-05/D-06).
3. **`pipeline.is_empty()` fast path** — already done.
4. **`sql_record_filter` outside pipeline** — correct; avoids spurious `parse_performance_metrics()` for filtered records.

**Remaining opportunities:**

- `record.tag.is_some() && !f.matches(pm.sql)` check in hot loop: `pm` is parsed before this check. If `record_sql` filter is configured and `include_pm = false`, `pm.sql` comes from `record.body()` (already cheaply available), so `parse_performance_metrics()` is skipped correctly. This branch is already handled.

- `pb_pending` batched progress bar updates (every 4096 records) — already done.

- `interrupted` check every 1024 records via `trailing_zeros() >= 10` — already done.

- **Regex engine**: `regex` crate uses a DFA/NFA hybrid. For simple patterns (e.g., exact string `SYSDBA`), `regex::Regex::is_match()` has higher overhead than `str::contains()`. Could selectively downgrade patterns that contain no regex metacharacters to literal `str::contains()`. Requires detection of "no metacharacters" at compile time. Moderate complexity, potential 10-30% speedup for literal-pattern-heavy configs.

- **`CompiledMetaFilters::should_keep()` field ordering**: Currently checks `usernames`, `client_ips`, `sess_ids`, `thrd_ids`, `statements`, `appnames`, trxids, `tags`. Hottest filter to put first is the one most likely to reject early. Cannot be statically optimized without runtime statistics. Not worth changing unless profiling shows a specific field dominates.

- **`params_buffer.clear()` per file**: Already `clear()` reuses allocation. No improvement possible.

### Files to Analyze for PERF-10

Profiling gate: need flamegraph on real 1.1GB file with filters enabled. Without profiling data, the only safe PERF-10 action is the literal-vs-regex detection for metadata patterns.

## PERF-11: CLI Startup / Config Loading

### Startup Cost Breakdown

From `main.rs` analysis, the startup sequence for `run` command:

1. **clap arg parse**: One-time, unavoidable. ~1-5ms typical.
2. **`check_for_updates_at_startup()`**: **Network I/O** — DNS + HTTP request. Could be 10-500ms. Already guarded by `!cli.quiet`. Could be moved to background thread.
3. **`Config::from_file()`**: `fs::read_to_string` + `toml::from_str`. TOML parsing of a small file (~2KB) is <1ms.
4. **`cfg.validate()` → `validate_regexes()`**: Calls `Regex::new()` for each pattern to validate. For N patterns, N regex compilations.
5. **`CompiledMetaFilters::from_meta()`** in `build_pipeline()`: N more regex compilations for the SAME patterns. This is the **double-compile problem**.
6. **`CompiledSqlFilters::from_sql_filters()`**: Same — compiled again.
7. **`SqllogParser::log_files()`**: Directory traversal + sorting. Usually <5ms.

### Optimization: Eliminate Double Regex Compilation

**Problem**: `validate_regexes()` compiles then discards. `from_meta()` compiles again.

**Fix**: Cache compiled regexes from validation. Two approaches:

**Approach A: validate() returns compiled structs (recommended)**

Change `validate()` to return pre-built `CompiledMetaFilters` and `CompiledSqlFilters`, stored on `Config` or passed into `handle_run()`. Avoids any duplication.

```rust
// config.rs
impl Config {
    pub fn validate_and_compile(&self) -> Result<CompiledFilters> {
        // validate + compile in one pass
    }
}
```

This changes the `handle_run` signature slightly but eliminates double compilation entirely.

**Approach B: lazy_static / OnceLock on FiltersFeature**

Add `OnceLock<CompiledMetaFilters>` to `FiltersFeature`. Shared between validate and build_pipeline calls. More complex, requires `Arc` for thread safety in parallel CSV path.

**Recommendation: Approach A.** Simpler, no shared state. Parallel CSV path creates per-thread pipelines anyway, so compiled structs need to be `Clone` or rebuilt — `Regex` is `Clone`, so compiled structs can be cloned cheaply (shared `Arc<Regex>` inside).

**Optimization: Background update check**

Move `check_for_updates_at_startup()` to a background thread with `std::thread::spawn`. Startup proceeds immediately; update notification printed after run completes or asynchronously.

```rust
let update_handle = if !cli.quiet && !is_update_or_completions_cmd {
    Some(std::thread::spawn(check_for_updates_at_startup))
} else {
    None
};
// ... rest of startup ...
// at end: update_handle.map(|h| h.join())
```

### Files to Modify for PERF-11

| File | Change | Type |
|------|--------|------|
| `src/config.rs` | Add `validate_and_compile()` returning `CompiledFilters` wrapper | **MODIFY** |
| `src/features/filters.rs` | Export a `CompiledFilters` struct bundling `CompiledMetaFilters` + `CompiledSqlFilters` | **MODIFY** |
| `src/cli/run.rs` | Accept pre-compiled filters from `validate_and_compile()` in `build_pipeline()` | **MODIFY** |
| `src/main.rs` | Background thread for update check | **MODIFY** |

## DEBT-01: SQLite Silent Errors

### Current State

In `sqlite.rs` `initialize()`:
```rust
} else if !self.append {
    let _ = conn.execute(&format!("DELETE FROM {}", self.table_name), []);
    // ^ Error silently discarded with `let _`
}
```

This is the only confirmed silent discard. The `DROP TABLE IF EXISTS` and `CREATE TABLE IF NOT EXISTS` paths DO propagate errors.

### Fix

Replace `let _ = conn.execute(...)` with proper error propagation:
```rust
conn.execute(&format!("DELETE FROM {}", self.table_name), [])
    .map_err(|e| Self::db_err(format!("clear table failed: {e}")))?;
```

Additionally, insert errors should be logged to the error log file, not just returned as `Err`. Currently, `export_one_preparsed()` returns `Err` which propagates up and terminates the run. For DEBT-01, the intent is: non-fatal insert errors should be written to the configured error log (`[error] file`) and processing continues, consistent with parse error behavior.

**This requires a design decision**: insert errors currently abort the export. DEBT-01 implies they should be logged and skipped. This mirrors how `Err` records from `parser.iter()` are handled (logged to `log::warn!`, processing continues).

### Integration Point

The error log path is in `cfg.error.file` (if an `[error]` section exists in config). The SQLite exporter currently has no reference to the error log path. Two options:

1. Pass error log path to `SqliteExporter::new()` and write directly (adds dependency).
2. Return a sentinel value from `export_one_preparsed()` indicating "logged but continuing" — but `Result<()>` already serves this; callers can log and continue.

**Recommendation**: In `handle_run()`, catch export errors from `exporter_manager.export_one_preparsed()` for SQLite path, log them to the error file (same mechanism as parse errors), increment a failed counter, and continue. This is the minimal change.

### Files to Modify for DEBT-01

| File | Change | Type |
|------|--------|------|
| `src/exporter/sqlite.rs` | Fix silent `let _` in `initialize()`; surface insert errors | **MODIFY** |
| `src/cli/run.rs` | Wrap `export_one_preparsed()` to log SQLite errors and continue (rather than abort) | **MODIFY** |

## DEBT-02: table_name SQL Injection

### Current State

`SqliteExporter` builds DDL and DML via string interpolation:
```rust
format!("DROP TABLE IF EXISTS {}", self.table_name)
format!("DELETE FROM {}", self.table_name)
format!("CREATE TABLE IF NOT EXISTS {table_name} (...)")
format!("INSERT INTO {table_name} VALUES (...)")
```

SQLite does not support `?` parameter binding for table names in DDL/DML. This is a structural constraint of the SQL wire protocol — parameters can only substitute values, not identifiers.

### Correct Fix: Whitelist Validation

Validate `table_name` at config parse time. A valid SQLite identifier contains only ASCII alphanumeric characters and underscores, does not start with a digit:

```rust
fn validate_table_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
```

Add this check to `SqliteExporter::validate()` in `config.rs`. If the name fails, return `ConfigError::InvalidValue`.

Additionally, quote the table name with double quotes in SQL strings as a defense-in-depth:
```rust
format!("DROP TABLE IF EXISTS \"{table_name}\"")
```

SQLite accepts double-quoted identifiers per the SQL standard (unlike MySQL backticks). This prevents any characters that slipped past validation from being parsed as SQL.

### Files to Modify for DEBT-02

| File | Change | Type |
|------|--------|------|
| `src/config.rs` | Add `validate_table_name()` helper; call in `SqliteExporter::validate()` | **MODIFY** |
| `src/exporter/sqlite.rs` | Wrap all `table_name` interpolations in double quotes | **MODIFY** |

## Component Interaction Map (v1.2)

```
Config (config.rs)
  └→ FiltersFeature (features/filters.rs)
       ├─ MetaFilters
       │    ├─ existing include fields (trxids, usernames, client_ips, ...)
       │    └─ [NEW FILTER-03] exclude_* fields
       ├─ IndicatorFilters
       ├─ SqlFilters (sql)         ← literal, pre-scan only
       └─ SqlFilters (record_sql)  ← literal, runtime (note: CompiledSqlFilters used in hot path)

  [PERF-11] validate_and_compile() → CompiledFilters
       ├─ CompiledMetaFilters  (from MetaFilters, includes new exclude_* Regex)
       └─ CompiledSqlFilters   (from record_sql SqlFilters)

cli/run.rs: handle_run()
  ├─ build_pipeline(cfg, compiled_filters)  ← [PERF-11] accepts pre-compiled
  │    └→ FilterProcessor { compiled_meta, exclude_meta, start_ts, end_ts }
  ├─ sql_record_filter: Option<&CompiledSqlFilters>  (unchanged)
  └─ process_log_file()
       └→ hot loop: pipeline + sql_record_filter
            └→ exporter_manager.export_one_preparsed()
                 └→ SqliteExporter  [DEBT-01: errors logged, not abort]
                                    [DEBT-02: table_name quoted]
```

## Build Order for v1.2 Phases

Based on dependencies:

1. **DEBT-02 first** — `table_name` validation is in `config.rs`; pure addition, no deps on other changes. Zero risk to filter or PERF work.

2. **DEBT-01** — SQLite error surfacing. Touches `sqlite.rs` and `run.rs` error handling. Independent of FILTER-03 and PERF.

3. **FILTER-03** — Extends `MetaFilters` + `CompiledMetaFilters`. Self-contained in `filters.rs`. No dependency on PERF-11 (can build with current double-compile; PERF-11 is an optimization on top).

4. **PERF-11** — Config/startup optimization. Requires knowing the final compiled filter shape after FILTER-03 adds `exclude_*` fields. Build after FILTER-03 to avoid rework.

5. **PERF-10** — Hot path optimization. Should be done after FILTER-03 (so exclude checking can be included in profiling) and after PERF-11 (so startup overhead is already reduced, isolating hot-loop from startup in measurements).

**Dependency graph:**
```
DEBT-02 → (none)
DEBT-01 → (none)
FILTER-03 → (none)
PERF-11 → FILTER-03 (needs final filter shape)
PERF-10 → FILTER-03, PERF-11 (profile after both)
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Adding Exclusion as a Separate Pipeline Stage

**What people do:** Create `ExcludeProcessor` implementing `LogProcessor`, add it to `Pipeline` after `FilterProcessor`.

**Why it's wrong:** Doubles the number of `process_with_meta()` calls in the hot loop. Splits the conceptual "filter" into two separated structs, making config validation and compilation more complex (two places to validate, two places to compile).

**Do this instead:** Extend `CompiledMetaFilters::should_keep()` with exclusion logic. All filter evaluation in one function call.

### Anti-Pattern 2: Compiling Regex in `SqlFilters::matches()`

**What people do:** Move regex compilation into the matches() call for simplicity.

**Why it's wrong:** `SqlFilters` (transaction-level) is called in pre-scan `par_iter()` across millions of records. Compiling regex per-call would be catastrophic.

**Do this instead:** Pre-compile all regexes at startup in the `Compiled*` structs. `SqlFilters` stays as literal-only, `CompiledSqlFilters` handles regex. This distinction is already in the codebase and must be preserved.

### Anti-Pattern 3: Storing `CompiledMetaFilters` in `FiltersFeature`

**What people do:** Add `OnceLock<CompiledMetaFilters>` to `FiltersFeature` for lazy compilation.

**Why it's wrong:** `FiltersFeature` is deserialized from config and may be cloned (pre-scan path clones `Config`). `OnceLock` is not `Clone`. `Arc<OnceLock<...>>` adds complexity. The double-compilation in PERF-11 is better fixed by returning compiled structs from `validate_and_compile()`.

**Do this instead:** `validate_and_compile()` on `Config` returns a separate `CompiledFilters` value that is passed into `handle_run()`. The config structs remain `Clone`-able and serde-deserializable.

### Anti-Pattern 4: Quoting table_name with Backticks

**What people do:** Use MySQL-style backtick quoting: `` `table_name` ``

**Why it's wrong:** SQLite uses double-quote for identifier quoting (per SQL standard). Backticks are accepted by SQLite in compatibility mode but are not standard and create confusion.

**Do this instead:** Use `"table_name"` (double-quote) in all SQL strings generated for SQLite.

## Sources

- Direct source inspection: `src/features/filters.rs`, `src/features/mod.rs`, `src/cli/run.rs`, `src/config.rs`, `src/exporter/sqlite.rs`, `src/exporter/mod.rs`, `src/main.rs`
- Confidence: HIGH — all claims verified from current code

---
*Architecture research for: sqllog2db v1.2 — FILTER-03, PERF-10, PERF-11, DEBT-01, DEBT-02*
*Researched: 2026-05-10*

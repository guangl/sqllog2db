# Testing Patterns

**Analysis Date:** 2026-04-17

## Test Framework

**Runner:**
- Built-in `cargo test` (Rust's standard test harness)
- No separate test runner configuration file

**Assertion Library:**
- Standard `assert!`, `assert_eq!`, `assert!(...is_ok())`, `assert!(...is_err())`
- No third-party assertion crates

**Benchmark Framework:**
- `criterion` 0.7 with `html_reports` feature
- Config: `[[bench]]` entries in `Cargo.toml`

**Run Commands:**
```bash
cargo test                        # Run all tests (unit + integration)
cargo test --release              # Run with optimizations (meaningful for throughput baseline)
cargo bench --bench bench_csv     # CSV export benchmark
cargo bench --bench bench_sqlite  # SQLite export benchmark
cargo bench --bench bench_filters # Filter pipeline benchmark
```

## Test File Organization

**Unit tests:** Co-located in source files inside `#[cfg(test)] mod tests { ... }` blocks. Present in 18 of the source files.

**Integration tests:** Single file `tests/integration.rs` — tests full CLI handler functions end-to-end.

**Benchmarks:** `benches/bench_csv.rs`, `benches/bench_sqlite.rs`, `benches/bench_filters.rs` — use `criterion` harness.

**Structure:**
```
sqllog2db/
├── src/
│   ├── config.rs            # #[cfg(test)] mod tests (33 unit tests)
│   ├── exporter/
│   │   ├── mod.rs           # #[cfg(test)] mod tests (18 unit tests)
│   │   ├── csv.rs           # #[cfg(test)] mod tests
│   │   └── sqlite.rs        # #[cfg(test)] mod tests
│   ├── features/
│   │   ├── mod.rs           # #[cfg(test)] mod tests (10 unit tests)
│   │   ├── filters.rs       # #[cfg(test)] mod tests
│   │   └── replace_parameters.rs  # #[cfg(test)] mod tests
│   ├── parser.rs            # #[cfg(test)] mod tests
│   ├── resume.rs            # #[cfg(test)] mod tests
│   └── ... (14 other files with unit tests)
├── tests/
│   └── integration.rs       # ~50 integration tests
└── benches/
    ├── bench_csv.rs
    ├── bench_sqlite.rs
    └── bench_filters.rs
```

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ── SectionName ──────────────────────────────────────────────
    #[test]
    fn test_function_name_scenario() {
        // arrange
        let cfg = Config::default();
        // act + assert
        assert!(cfg.validate().is_ok());
    }
}
```

**Patterns:**
- Section divider comments group related tests: `// ── validate ─────`, `// ── apply_overrides ─────`
- Test names follow `test_<subject>_<scenario>`: `test_validate_empty_csv_file`, `test_apply_overrides_unknown_key_returns_error`
- Each test is narrow — one behavior per function
- No `#[should_panic]` — errors are checked via `assert!(result.is_err())`

## Mocking

**Framework:** None — no mock crates used (`mockall`, `mockito`, etc. are absent from `Cargo.toml`)

**Strategy:**
- Integration tests use real filesystem via `tempfile::TempDir` — all I/O is against actual temp directories
- `DryRunExporter` is the production no-op for testing pipeline logic without file I/O
- The `interrupted` flag (`Arc<AtomicBool>`) is passed directly to `handle_run` — tests set it to `true` to simulate Ctrl-C

**What to Mock:** Nothing — the project avoids mocking in favor of real I/O through temp directories.

## Fixtures and Factories

**Test log generation:**
```rust
fn write_test_log(path: &std::path::Path, count: usize) {
    use std::fmt::Write as _;
    let mut buf = String::with_capacity(count * 180);
    for i in 0..count {
        writeln!(
            buf,
            "2025-01-15 10:30:28.001 (EP[0] sess:0x{i:04x} user:TESTUSER trxid:{i} stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT * FROM t WHERE id={i}. EXECTIME: {exec}(ms) ROWCOUNT: {rows}(rows) EXEC_ID: {i}.",
            exec = (i * 13) % 1000,
            rows = i % 100,
        ).unwrap();
    }
    std::fs::write(path, buf).unwrap();
}
```

**Config factory:**
```rust
fn make_run_config(log_dir: &std::path::Path, csv_file: &std::path::Path) -> Config {
    Config {
        sqllog: SqllogConfig { path: log_dir.to_str().unwrap().to_string() },
        exporter: ExporterConfig {
            csv: Some(CsvExporter { file: csv_file.to_str().unwrap().to_string(), overwrite: true, append: false }),
            ..Default::default()
        },
        ..Default::default()
    }
}
```

**Location:** Helper functions are defined at the top of each test file/module, not shared across files.

**Temp directory pattern:** All integration tests use `tempfile::TempDir::new().unwrap()` — directories are automatically cleaned up on drop.

## Coverage

**Requirements:** No explicit coverage threshold configured. Target ~80% mentioned in recent commit `08560d2`.

**View Coverage:**
```bash
# Install cargo-tarpaulin or use llvm-cov:
cargo llvm-cov --html
```

## Test Types

**Unit Tests:**
- Scope: Single struct/function behavior in isolation
- Examples: `ExportStats::total()`, `strip_ip_prefix()`, `f32_ms_to_i64()`, `ReplaceParametersConfig::placeholder_override()`, `FieldMask::is_active()`
- Location: `#[cfg(test)] mod tests` in the same file as the tested code

**Integration Tests:**
- Scope: Full CLI handler functions (`handle_run`, `handle_stats`, `handle_digest`, `handle_init`, `handle_validate`) exercised end-to-end with real files
- Location: `tests/integration.rs`
- What they cover:
  - `handle_run`: dry-run, real CSV export, limit, interrupt, resume (skip and reprocess), parallel multi-file, filter pipeline
  - `handle_stats`: empty dir, nonexistent dir, group-by, bucket (hour/minute), JSON output, verbose mode
  - `handle_digest`: empty dir, sort modes, top-N, min_count filter, JSON output, fingerprint aggregation
  - `handle_init`: creates file, fails without force, force-overwrite, EN/ZH templates
  - `handle_validate`: all config branches (CSV, SQLite, filters enabled/disabled, replace_parameters)

**Performance Baseline (in integration tests):**
- `test_csv_throughput_baseline` in `tests/integration.rs` asserts minimum throughput
  - Debug build minimum: 30,000 rec/s
  - Release build minimum: 500,000 rec/s

**Benchmarks (criterion):**
- `benches/bench_csv.rs`: CSV export at 1k / 10k / 50k records with `Throughput::Elements`
- `benches/bench_sqlite.rs`: SQLite export at similar scales
- `benches/bench_filters.rs`: Filter pipeline throughput
- Baselines saved under `benches/baselines/` for regression comparison

## Common Patterns

**Async Testing:** Not applicable — the codebase is fully synchronous.

**Error Testing:**
```rust
#[test]
fn test_validate_empty_csv_file() {
    let mut cfg = default_config();
    cfg.exporter.csv = Some(CsvExporter { file: "  ".into(), ..CsvExporter::default() });
    assert!(cfg.validate().is_err());
}
```

**Output Verification:**
```rust
handle_run(&cfg, None, false, true, &interrupted, 80, false, None, 1).unwrap();
let content = std::fs::read_to_string(&csv_file).unwrap();
let data_lines = content.lines().count().saturating_sub(1); // minus header
assert!(data_lines <= 5, "expected ≤5 records, got {data_lines}");
```

**Panic-safety tests (no-panic contracts):**
```rust
// Should not panic — prints an error and returns
handle_stats(&cfg, true, false, None, false, &[], None, None);
```
Many integration tests verify that invalid inputs (nonexistent dirs, bad field names, disabled filters) do not panic — they just print an error and return gracefully.

---

*Testing analysis: 2026-04-17*

# Technology Stack

**Analysis Date:** 2026-04-17

## Languages

**Primary:**
- Rust (Edition 2024) - entire codebase; minimum supported version 1.85

**Secondary:**
- TOML - configuration files (`config.toml`, `rustfmt.toml`, Cargo manifests)
- YAML - CI/CD pipelines (`.github/workflows/`)

## Runtime

**Environment:**
- Native binary — no runtime required; compiled via `cargo build --release`

**Package Manager:**
- Cargo (Rust standard)
- Lockfile: `Cargo.lock` present and committed (binary crate)

## Frameworks

**CLI:**
- `clap` 4.6.0 (features: derive, env) — argument parsing and subcommands
- `clap_complete` 4.6 — shell completion generation
- `clap_mangen` 0.3 — man page generation

**Testing:**
- Built-in `cargo test` with `#[test]` and `#[cfg(test)]` blocks
- `criterion` 0.7 (features: html_reports) — benchmark harness; three benches: `bench_csv`, `bench_sqlite`, `bench_filters`

**Build/Dev:**
- `rustfmt` with `rustfmt.toml` (max_width=100, tab_spaces=4, edition="2021")
- `clippy` with strict lint config in `Cargo.toml` (`pedantic`, `cargo`, `-D warnings`)

## Key Dependencies

**Critical:**
- `dm-database-parser-sqllog` 0.9.1 — proprietary/internal crate that parses DaMeng SQL log lines into `Sqllog` records; the single parsing dependency
- `rusqlite` 0.39.0 (features: bundled, vtab, csvtab) — SQLite exporter; bundled means no system SQLite required
- `rayon` 1.11 — parallel pre-scan of log files for transaction ID collection

**Performance:**
- `mimalloc` 0.1 — global allocator replacing system allocator; set via `#[global_allocator]` in `src/main.rs`
- `itoa` 1.0 — zero-allocation integer-to-ASCII for CSV formatting
- `ryu` 1 — zero-allocation float-to-ASCII
- `memchr` 2 — SIMD-accelerated byte search used in CSV escape logic (`src/exporter/csv.rs`)
- `compact_str` 0.9 — small-string optimization
- `smallvec` 1 (features: union) — stack-allocated vectors
- `ahash` 0.8 — fast non-cryptographic hash map

**Serialization:**
- `serde` 1.0.228 (features: derive) — config deserialization
- `toml` 1.1.2 (features: serde) — TOML config parsing
- `serde_json` 1.0.149 — JSON output for stats/digest commands

**Encoding:**
- `encoding_rs` 0.8 — GB18030/GBK log file re-encoding to UTF-8

**Utilities:**
- `chrono` 0.4.44 (features: clock) — timestamp parsing and date-range filtering
- `glob` 0.3 — file path glob expansion for `sqllog.path`
- `indicatif` 0.18 — progress bar during run/stats/digest
- `ctrlc` 3 — Ctrl+C signal handler for graceful shutdown
- `env_logger` 0.11.10 — console logging for non-run commands
- `log` 0.4.29 — logging facade
- `thiserror` 2.0.18 — structured error types with `#[derive(Error)]`
- `self_update` 0.44.0 (features: reqwest, rustls, compression-flate2) — GitHub release self-update

**Dev only:**
- `tempfile` 3.27.0 — temporary directories in tests
- `criterion` 0.7 — benchmarks

## Configuration

**Environment:**
- Application config via TOML file (`config.toml` by default, overridable via `-c` flag)
- Config sections: `[sqllog]`, `[logging]`, `[exporter.csv]`, `[exporter.sqlite]`, `[features]`, `[resume]`
- CLI `--set key=value` overrides any config key at runtime
- No `.env` file usage; no environment variable config beyond `clap`'s `env` feature for CLI flags

**Build:**
- `Cargo.toml` `[profile.release]`: `opt-level=3`, `lto="fat"`, `codegen-units=1`, `panic="abort"`, `strip="symbols"`
- `rustfmt.toml`: max_width=100, tab_spaces=4

## Platform Requirements

**Development:**
- Rust 1.85+ (edition 2024)
- Cargo (any recent version)
- `rustfmt` and `clippy` for lint/format checks

**Production:**
- Self-contained statically-linked binary (SQLite bundled)
- No external runtime dependencies
- Targets: `aarch64-apple-darwin` confirmed; GitHub Releases via `self_update` imply multi-platform builds in CI

---

*Stack analysis: 2026-04-17*

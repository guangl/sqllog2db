# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build (default = CSV only)
cargo build --release

# Build with optional features
cargo build --release --features "jsonl sqlite filters"
cargo build --release --features full

# Test
cargo test
cargo test --all-features

# Lint (must pass with no warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Format
cargo fmt

# CLI usage
cargo run -- init -o config.toml --force
cargo run -- validate -c config.toml
cargo run -- run -c config.toml
```

## Architecture

**sqllog2db** parses DaMeng (达梦) database SQL log files and exports them to CSV, JSONL, or SQLite. It streams log records through an optional processing pipeline and writes to a single configured exporter.

### Data Flow

```
Input .log files (sqllogs/)
    ↓ SqllogParser        — discovers files (src/parser.rs)
    ↓ dm-database-parser-sqllog  — parses each line into Sqllog records
    ↓ Pipeline            — optional filters (src/features/)
    ↓ ExporterManager     — routes to active exporter (src/exporter/)
    ↓ Output (CSV / JSONL / SQLite)
```

### Key Modules

- **`cli/run.rs`** — main orchestration: loads config, builds pipeline, pre-scans for transaction filters, processes files in 5000-record batches
- **`exporter/mod.rs`** — `Exporter` trait + `ExporterManager` factory; only one exporter is active per run (priority: CSV > JSONL > SQLite)
- **`features/mod.rs`** — `LogProcessor` trait + `Pipeline`; `pipeline.is_empty()` enables a zero-overhead fast path when no filters are configured
- **`features/filters.rs`** — feature-gated (`--features filters`); two-pass design: pre-scan finds matching transaction IDs, main pass applies all filters
- **`config.rs`** — all config structs with serde deserialization and validation

### Features (compile-time)

| Feature | Adds |
|---------|------|
| *(default)* | CSV export only |
| `jsonl` | JSONL export |
| `sqlite` | SQLite export via rusqlite |
| `filters` | Filter pipeline |
| `full` | All of the above |

### Performance Design

- Single-threaded streaming — constant memory regardless of file size
- 16MB `BufWriter` + `itoa` crate for zero-allocation CSV formatting
- `pipeline.is_empty()` check in the hot loop avoids filter overhead when disabled
- Binary: LTO + strip + `panic=abort` + `opt-level=z`
- Benchmark: ~1.55M records/sec on a 1.1GB file

### Error Handling

Parse errors are not fatal — they are written to the configured error log file (`[error] file`) and processing continues. Structured errors use `thiserror`; all variants include path/reason context.

# Coding Conventions

**Analysis Date:** 2026-04-17

## Naming Patterns

**Files:**
- Modules use `snake_case` filenames: `replace_parameters.rs`, `sql_fingerprint.rs`, `show_config.rs`
- Submodule directories have a `mod.rs` entry point: `src/cli/mod.rs`, `src/exporter/mod.rs`, `src/features/mod.rs`

**Structs:**
- `PascalCase` for all types: `SqllogParser`, `ExportStats`, `DryRunExporter`, `FieldMask`, `MetaFilters`
- Config structs are suffixed with their section name: `SqllogConfig`, `LoggingConfig`, `ExporterConfig`, `FeaturesConfig`
- Exporter implementations are suffixed `Exporter`: `CsvExporter`, `SqliteExporter`, `DryRunExporter`

**Traits:**
- Named after the capability, not the implementor: `Exporter`, `LogProcessor`

**Functions:**
- `snake_case` throughout: `handle_run`, `from_config`, `write_record_preparsed`, `strip_ip_prefix`
- CLI handler functions are prefixed `handle_`: `handle_run`, `handle_init`, `handle_validate`, `handle_stats`, `handle_digest`
- Constructor functions follow Rust convention: `new()` for bare construction, `from_config(cfg)` for config-driven construction

**Variables:**
- Descriptive `snake_case` names throughout; single-letter names are absent except in closures
- Boolean flags are named with an adjective: `overwrite`, `append`, `normalize`, `interrupted`

**Constants:**
- `UPPER_SNAKE_CASE` for `const` and `static`: `FIELD_NAMES`, `LOG_LEVELS`, `PREFIX`

## Code Style

**Formatting:**
- `cargo fmt` (rustfmt) with default settings; enforced in CI
- Trailing commas in multi-line struct/enum literals

**Linting:**
- `cargo clippy` with `pedantic` + `cargo` lint groups enabled (`-D warnings`)
- Allowed exceptions declared in `Cargo.toml` under `[lints.clippy]`:
  - `module_name_repetitions`, `type_complexity`, `similar_names`, `too_many_arguments`, `too_many_lines`, `items_after_statements` are all allowed
- `#[expect(clippy::cast_possible_truncation, reason = "...")]` is used at call sites with an explanation string — prefer `#[expect]` over `#[allow]` so the suppression errors if the lint no longer fires

**Attributes used frequently:**
- `#[inline]` on hot-path methods: `write_record_preparsed`, `export_one_preparsed`, `run_with_meta`, `is_active`
- `#[must_use]` on pure functions that return meaningful values: `new()`, `from_config()`, `is_empty()`, `total()`
- `#[derive(Debug)]` on all public types

## Import Organization

**Order (observed pattern):**
1. Standard library (`std::`)
2. External crates (`log`, `serde`, `thiserror`, `ahash`, etc.)
3. Internal crates (`crate::`, `super::`)

**Re-exports in `mod.rs`:**
- Submodule items are selectively re-exported with `pub use` in `mod.rs` files:
  - `src/features/mod.rs`: `pub use filters::FiltersFeature`, `pub use replace_parameters::compute_normalized`, `pub use sql_fingerprint::fingerprint`
  - `src/exporter/mod.rs`: `pub use csv::CsvExporter`, `pub use sqlite::SqliteExporter`
- `src/lib.rs` re-exports the entire exporter module with `pub use exporter::*`

## Error Handling

**Type alias:** `src/error.rs` defines `pub type Result<T> = std::result::Result<T, Error>` used project-wide.

**Error hierarchy:**
- Top-level `Error` enum in `src/error.rs` wraps sub-errors via `#[from]`:
  - `Error::Config(ConfigError)`
  - `Error::File(FileError)`
  - `Error::Parser(ParserError)`
  - `Error::Export(ExportError)`
  - `Error::Update(UpdateError)`
  - `Error::Io(io::Error)`
  - `Error::Interrupted`
- All sub-error variants include context fields (`path: PathBuf`, `reason: String`) — never bare strings

**Pattern at call sites:**
- `.map_err(|e| Error::Config(ConfigError::ParseFailed { path: path.to_path_buf(), reason: e.to_string() }))` — inline, not via helper functions
- `?` operator used throughout for propagation
- Parse errors from the underlying parser library are **non-fatal**: written to the error log file and processing continues

**Validation:**
- Config structs implement a `validate(&self) -> Result<()>` method that returns structured `ConfigError` variants

## Logging

**Framework:** `log` crate macros (`log::info!`, `log::debug!`, `log::warn!`)

**Patterns:**
- `info!` for lifecycle events: initializing, finalizing exporters, using a specific exporter path
- `debug!` for per-file scan details in `src/parser.rs`
- `warn!` for recoverable issues (parse errors, encoding fallbacks)
- Log calls are typically in handler functions (`src/cli/run.rs`) and the exporter manager, not in hot-path loops

## Comments

**Language:**
- Chinese comments are used for domain-specific DaMeng concepts and performance rationale
- English comments/doc comments for public API

**Doc comments:**
- `///` used on all public structs, traits, and methods with non-obvious behavior
- Performance rationale is documented inline (e.g., why `CompactString`, why `AHashSet`, why `#[inline]`)

**Section dividers:**
- `// ── SectionName ─────────────────────────────────────────────` style used to visually separate sections within long files

## Module Organization Style

- Business logic lives in leaf modules (`src/features/filters.rs`, `src/exporter/csv.rs`)
- `mod.rs` files define traits, shared types, and re-exports — not implementation
- CLI handlers are one-per-file under `src/cli/`: `run.rs`, `stats.rs`, `digest.rs`, `init.rs`, etc.
- Config structs live in `src/config.rs` alongside their `validate()` and `apply_overrides()` methods
- `src/error.rs` is standalone, imported by every other module

## Common Patterns

**Enum dispatch instead of `Box<dyn Trait>`:**
- `ExporterKind` enum (`Csv`, `Sqlite`, `DryRun`) wraps concrete exporters; all dispatch is via `match`, not vtable, enabling inlining of the hot path (`src/exporter/mod.rs`)

**Zero-overhead fast path:**
- `pipeline.is_empty()` checked in the hot loop before calling `run_with_meta()`; avoids all filter overhead when no filters are configured (`src/cli/run.rs`, `src/features/mod.rs`)

**Pre-parsed data passing:**
- `parse_meta()` and `parse_performance_metrics()` are called once per record in the orchestrator; the results (`MetaParts`, `PerformanceMetrics`) are threaded through `process_with_meta` and `export_one_preparsed` to eliminate redundant parsing

**`from_config` constructors:**
- Every exporter and pipeline component takes a typed config struct: `CsvExporter::from_config(&config::CsvExporter)`, `SqliteExporter::from_config(&config::SqliteExporter)`

**Default + struct update syntax:**
- Tests and integration code build `Config` using `Config { field: value, ..Default::default() }` extensively

---

*Convention analysis: 2026-04-17*

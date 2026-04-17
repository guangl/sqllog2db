# Codebase Structure

**Analysis Date:** 2026-04-17

## Directory Layout

```
sqllog2db/
├── src/                    # All Rust source code
│   ├── main.rs             # Binary entry point, CLI dispatch, exit codes
│   ├── lib.rs              # Re-exports all modules (for integration tests)
│   ├── config.rs           # Config structs, TOML loading, validation, --set overrides
│   ├── error.rs            # Typed error hierarchy (thiserror)
│   ├── parser.rs           # SqllogParser: file/dir/glob → Vec<PathBuf>
│   ├── resume.rs           # ResumeState: fingerprint-based incremental processing
│   ├── logging.rs          # File-based log rotation setup
│   ├── color.rs            # ANSI color helpers
│   ├── lang.rs             # i18n: auto-detect language, apply zh help strings
│   ├── cli/                # Per-subcommand handlers + CLI option structs
│   │   ├── mod.rs          # pub mod declarations
│   │   ├── opts.rs         # Cli struct, Commands enum (clap derive)
│   │   ├── run.rs          # handle_run(): main export orchestration
│   │   ├── stats.rs        # handle_stats(): record counting + aggregation
│   │   ├── digest.rs       # handle_digest(): SQL fingerprint aggregation
│   │   ├── init.rs         # handle_init(): generate default config.toml
│   │   ├── validate.rs     # handle_validate(): config validation report
│   │   ├── show_config.rs  # handle_show_config(): print effective config
│   │   ├── preflight.rs    # Pre-run checks: log dir exists, output is writable
│   │   └── update.rs       # handle_update(): self-update via GitHub releases
│   ├── exporter/           # Output writers
│   │   ├── mod.rs          # Exporter trait, ExporterKind enum, ExporterManager, DryRunExporter
│   │   ├── csv.rs          # CsvExporter: 16MB BufWriter + itoa/ryu zero-alloc formatting
│   │   └── sqlite.rs       # SqliteExporter: rusqlite batch inserts
│   └── features/           # Processing pipeline and record transformations
│       ├── mod.rs          # LogProcessor trait, Pipeline, FeaturesConfig, FieldMask, ReplaceParametersConfig
│       ├── filters.rs      # FiltersFeature, MetaFilters, IndicatorFilters, SqlFilters, RecordMeta
│       ├── replace_parameters.rs  # compute_normalized(): substitute ? / :N params
│       └── sql_fingerprint.rs     # fingerprint(): structural SQL fingerprint
├── tests/
│   └── integration.rs      # Integration tests (uses lib.rs re-exports)
├── benches/
│   ├── bench_csv.rs        # criterion benchmark: CSV export throughput
│   ├── bench_sqlite.rs     # criterion benchmark: SQLite export throughput
│   └── bench_filters.rs    # criterion benchmark: filter pipeline throughput
├── Cargo.toml              # Package manifest, dependencies, release profile
├── Cargo.lock              # Pinned dependency versions
├── config.toml             # Example / default config (excluded from crate package)
├── CLAUDE.md               # Project instructions for Claude Code
├── CHANGELOG.md            # Version history
├── rustfmt.toml            # Formatter config
└── cliff.toml              # git-cliff changelog generation config
```

## Directory Purposes

**`src/cli/`:**
- Purpose: One file per subcommand handler plus the clap option structs
- Contains: Handler functions (`handle_*`), clap `Cli` + `Commands` derive structs, preflight checks, update logic
- Key files: `opts.rs` (all CLI flags/subcommands), `run.rs` (main export loop)

**`src/exporter/`:**
- Purpose: Pluggable output backends behind the `Exporter` trait; `ExporterKind` enum provides static dispatch
- Contains: CSV writer, SQLite writer, dry-run stub, manager factory
- Key files: `mod.rs` (trait + manager), `csv.rs`, `sqlite.rs`

**`src/features/`:**
- Purpose: All optional record processing: filtering, SQL normalization, fingerprinting, field projection
- Contains: `Pipeline` with `LogProcessor` trait, filter config structs, parameter replacement, fingerprinting
- Key files: `mod.rs` (trait/pipeline/config), `filters.rs` (all filter types)

**`tests/`:**
- Purpose: Integration tests that exercise the full public API via `lib.rs` re-exports
- Contains: `integration.rs` — end-to-end test scenarios using `tempfile` directories

**`benches/`:**
- Purpose: criterion benchmarks for hot paths
- Contains: CSV throughput, SQLite throughput, filter pipeline overhead

## Key File Locations

**Entry Points:**
- `src/main.rs`: Binary entry point — `fn main()` calls `fn run() -> Result<()>`; exit code mapping
- `src/lib.rs`: Library root — re-exports all modules for integration test access

**Configuration:**
- `src/config.rs`: All config structs (`Config`, `SqllogConfig`, `LoggingConfig`, `ExporterConfig`, `CsvExporter`, `SqliteExporter`, `ResumeConfig`); `Config::from_file()`, `Config::validate()`, `Config::apply_overrides()`
- `config.toml`: Example configuration file at project root (not committed to crate package)

**Core Logic:**
- `src/cli/run.rs`: Main export orchestration — file loop, pipeline, pre-scan, parallel CSV path, resume state
- `src/features/mod.rs`: `LogProcessor` trait, `Pipeline`, `FieldMask`
- `src/exporter/mod.rs`: `Exporter` trait, `ExporterManager`, `ExporterKind`

**Error Handling:**
- `src/error.rs`: All error types (`Error`, `ConfigError`, `ParserError`, `ExportError`, `FileError`, `UpdateError`)

**Testing:**
- `tests/integration.rs`: Integration tests
- Inline `#[cfg(test)] mod tests` blocks present in every source module

## Naming Conventions

**Files:**
- Modules use `snake_case.rs` (e.g., `replace_parameters.rs`, `sql_fingerprint.rs`)
- One module per file; module name matches file name

**Directories:**
- Plural noun for collections of related modules: `cli/`, `exporter/`, `features/`
- Each directory has a `mod.rs` with `pub mod` declarations

**Functions:**
- Handler functions: `handle_{subcommand}()` pattern (e.g., `handle_run`, `handle_stats`, `handle_digest`)
- Constructor methods: `new()` for simple construction, `from_config()` when built from a config struct, `from_path()` when built from a file path
- Boolean query methods: `is_empty()`, `is_processed()`, `has_filters()`, `has_any()`

**Types:**
- Structs: `PascalCase` (e.g., `SqllogParser`, `ExporterManager`, `ResumeState`)
- Enums: `PascalCase` variants (e.g., `ExporterKind::Csv`, `Commands::Run`)
- Traits: `PascalCase` (e.g., `LogProcessor`, `Exporter`)
- Config structs: named after their TOML section + `Config` or `Exporter` suffix (e.g., `LoggingConfig`, `CsvExporter`)
- Error enums: domain + `Error` suffix (e.g., `ConfigError`, `ExportError`)

**Constants:**
- `SCREAMING_SNAKE_CASE` (e.g., `EXIT_CONFIG`, `FIELD_NAMES`, `FieldMask::ALL`)

## Where to Add New Code

**New subcommand:**
- Add variant to `Commands` in `src/cli/opts.rs`
- Add handler file `src/cli/{name}.rs` with `pub fn handle_{name}()`
- Add `pub mod {name};` to `src/cli/mod.rs`
- Dispatch from `match &cli.command` in `src/main.rs`

**New filter type:**
- Add filter struct to `src/features/filters.rs`
- Add field to `FiltersFeature` in `src/features/filters.rs`
- Implement matching logic in a `matches()` or `should_keep()` method
- Wire into `FilterProcessor::process_with_meta()` in `src/cli/run.rs`
- Add TOML config field to `FiltersFeature` (serde `Deserialize`)

**New exporter:**
- Add implementation file `src/exporter/{name}.rs` implementing `Exporter` trait
- Add variant to `ExporterKind` enum in `src/exporter/mod.rs`
- Add delegation arms in all `ExporterKind` match expressions
- Add config struct to `src/config.rs` and field to `ExporterConfig`
- Add selection branch in `ExporterManager::from_config()` (priority order: first configured wins)

**New feature processor:**
- Implement `LogProcessor` trait (`src/features/mod.rs`)
- Add processor in `build_pipeline()` in `src/cli/run.rs`
- Add config struct to `src/features/mod.rs` or `src/features/{name}.rs`
- Add field to `FeaturesConfig` in `src/features/mod.rs`

**New config key for `--set` override:**
- Add arm to `apply_one()` match in `src/config.rs`

**Utilities / helpers:**
- Shared output formatting helpers: `src/color.rs`
- Shared exporter utilities (IP stripping, float conversion): `src/exporter/mod.rs` (private `pub(super)` functions)

## Special Directories

**`target/`:**
- Purpose: Cargo build artifacts
- Generated: Yes
- Committed: No

**`sqllogs/`:**
- Purpose: Default input directory for DM SQL log files; excluded from crate package
- Generated: No (user-supplied)
- Committed: No (`.gitignore`)

**`outputs/`:**
- Purpose: Default CSV output directory
- Generated: Yes (created by exporter on first run)
- Committed: No

**`export/`:**
- Purpose: Default SQLite output directory
- Generated: Yes
- Committed: No

**`logs/`:**
- Purpose: Application log files (`logs/sqllog2db.log`)
- Generated: Yes
- Committed: No

**`.planning/`:**
- Purpose: GSD planning documents (phases, codebase maps)
- Generated: By planning tooling
- Committed: Yes

---

*Structure analysis: 2026-04-17*

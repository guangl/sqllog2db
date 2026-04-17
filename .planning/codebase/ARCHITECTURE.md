# Architecture

**Analysis Date:** 2026-04-17

## Pattern Overview

**Overall:** Streaming pipeline CLI

**Key Characteristics:**
- Single binary, single-threaded streaming by default (constant memory regardless of input size)
- Optional parallel mode: per-file rayon tasks, each writing a temp CSV, concatenated in order at the end
- Two-pass design for transaction-level filters: pre-scan pass collects matching `trxid`s, main pass applies all filters
- Zero-overhead fast path: `pipeline.is_empty()` check skips filter overhead entirely when no filters are configured
- Single active exporter per run (CSV takes priority over SQLite); exporter is selected at startup from config

## Layers

**CLI / Orchestration:**
- Purpose: Parse CLI args, load config, build pipeline, dispatch to subcommand handlers
- Location: `src/main.rs`, `src/cli/`
- Contains: Subcommand dispatch (`main.rs`), per-command handlers (`cli/run.rs`, `cli/stats.rs`, `cli/digest.rs`, etc.), CLI option structs (`cli/opts.rs`)
- Depends on: Config, Pipeline, ExporterManager, SqllogParser
- Used by: End user (binary entry point)

**Configuration:**
- Purpose: Load, validate, and override TOML config; typed structs for every config section
- Location: `src/config.rs`
- Contains: `Config`, `SqllogConfig`, `LoggingConfig`, `ExporterConfig`, `CsvExporter`, `SqliteExporter`, `ResumeConfig`
- Depends on: `features::FeaturesConfig` (re-exported into config)
- Used by: All layers

**File Discovery (Parser):**
- Purpose: Resolve `sqllog.path` (file, directory, or glob) into a sorted `Vec<PathBuf>`
- Location: `src/parser.rs`
- Contains: `SqllogParser`
- Depends on: `glob` crate, stdlib fs
- Used by: `cli/run.rs`, `cli/stats.rs`, `cli/digest.rs`

**Features / Pipeline:**
- Purpose: Optional record-level and transaction-level filtering; parameter normalization; field projection
- Location: `src/features/mod.rs`, `src/features/filters.rs`, `src/features/replace_parameters.rs`, `src/features/sql_fingerprint.rs`
- Contains: `LogProcessor` trait, `Pipeline`, `FeaturesConfig`, `FiltersFeature`, `FieldMask`, `ReplaceParametersConfig`, `compute_normalized`, `fingerprint`
- Depends on: `dm-database-parser-sqllog` (external crate)
- Used by: `cli/run.rs`

**Exporter:**
- Purpose: Write parsed records to CSV or SQLite; manage output lifecycle (initialize → stream → finalize)
- Location: `src/exporter/mod.rs`, `src/exporter/csv.rs`, `src/exporter/sqlite.rs`
- Contains: `Exporter` trait, `ExporterKind` enum (static dispatch), `ExporterManager`, `CsvExporter`, `SqliteExporter`, `DryRunExporter`, `ExportStats`
- Depends on: `rusqlite`, `itoa`, `ryu` crates
- Used by: `cli/run.rs`

**Resume / State:**
- Purpose: Track processed files by fingerprint (path + size + mtime) for incremental runs
- Location: `src/resume.rs`
- Contains: `ResumeState`, `ProcessedFile`
- Depends on: `chrono`, `toml`, `serde`
- Used by: `cli/run.rs`, `cli/stats.rs`, `cli/digest.rs`

**Support Modules:**
- `src/error.rs` — typed error hierarchy (`Error`, `ConfigError`, `ParserError`, `ExportError`, `FileError`, `UpdateError`)
- `src/logging.rs` — file-based log rotation setup
- `src/color.rs` — ANSI color helpers respecting `--no-color` / `NO_COLOR`
- `src/lang.rs` — i18n: detect language from `LANG` env or `--lang` flag, apply zh help strings to clap command
- `src/lib.rs` — re-exports all modules (enables integration test crate to import them)

## Data Flow

**Normal export run (`sqllog2db run`):**

1. `main()` parses CLI args via clap, detects language, initializes color
2. `load_config()` reads `config.toml` via `Config::from_file()` + `cfg.validate()`
3. `cli::run::handle_run()` is called with the final `Config`
4. `SqllogParser::new(cfg.sqllog.path).log_files()` returns sorted `Vec<PathBuf>`
5. **Optional pre-scan** (only when transaction-level filters present): `scan_for_trxids_by_transaction_filters()` runs rayon parallel scan across all files, collecting matching `trxid`s; results are merged back into config's `trxids` allowlist
6. `build_pipeline(cfg)` constructs a `Pipeline` with zero or more `LogProcessor` instances
7. For each log file (sequential or parallel CSV path):
   - `LogParser::from_path(file)` opens the file (external `dm-database-parser-sqllog` crate)
   - Hot loop: for each `Ok(record)` from `parser.iter()`:
     - If `pipeline.is_empty()` → bypass all filter logic (zero-overhead fast path)
     - Else → `pipeline.run_with_meta(&record, &meta)` → boolean keep/discard
     - SQL record-level filter applied separately on `pm.sql` (DML records only)
     - `compute_normalized()` builds `normalized_sql` if `do_normalize && field active`
     - `exporter_manager.export_one_preparsed(&record, &meta, &pm, normalized)` writes the record
8. `exporter_manager.finalize()` flushes buffers and closes output files
9. If `--resume`: `ResumeState::mark_processed()` + `state.save()`

**Parallel CSV path** (multi-file + `--jobs > 1` + CSV exporter + no `--limit`):

1. Each file assigned to a rayon worker; each worker creates its own temp `CsvExporter` writing to a temp file in `.{stem}_parts_{pid}/`
2. After all workers complete, `concat_csv_parts()` concatenates temp CSVs in original file order into the final output, skipping duplicate headers
3. Temp directory is removed

**Stats / Digest commands:**
- Same file discovery, no pipeline/exporter; stream records directly into in-memory counters or fingerprint maps; print results to stdout

## Key Abstractions

**`LogProcessor` trait** (`src/features/mod.rs`):
- Purpose: Pluggable record filter; return `true` to keep, `false` to discard
- Key methods: `process(&self, record)`, `process_with_meta(&self, record, meta)` (hot-path override to reuse pre-parsed meta)
- Implementations: `FilterProcessor` (in `cli/run.rs`) — wraps `FiltersFeature`

**`Pipeline`** (`src/features/mod.rs`):
- Purpose: Ordered chain of `LogProcessor` boxes; short-circuits on first `false`
- Key method: `run_with_meta()` — shares one `parse_meta()` call across all processors
- Fast path: `pipeline.is_empty()` checked before calling `run_with_meta()` in the hot loop

**`Exporter` trait** (`src/exporter/mod.rs`):
- Purpose: Lifecycle interface for output writers (`initialize` / `export_one_preparsed` / `finalize`)
- Hot-path method: `export_one_preparsed(sqllog, meta, pm, normalized)` — accepts pre-parsed structs to avoid re-parsing inside the exporter
- Static dispatch: `ExporterKind` enum wraps `CsvExporter | SqliteExporter | DryRunExporter`; no vtable overhead in hot loop

**`ExporterManager`** (`src/exporter/mod.rs`):
- Purpose: Factory + single active exporter; created from `Config` or directly from a `CsvExporter` (parallel path)
- Selection priority: CSV → SQLite (first configured wins)

**`FieldMask`** (`src/features/mod.rs`):
- Purpose: `u16` bitmask for 15 output fields; bit `i` = 1 means field `i` is exported
- Used by exporters to skip writing disabled fields; also gates `compute_normalized()` call

**`ResumeState`** (`src/resume.rs`):
- Purpose: Persist fingerprints (path + size + mtime) of processed files to a TOML state file; checked before processing each file when `--resume` is active

## Entry Points

**Binary entry point:**
- Location: `src/main.rs` — `fn main()` → `fn run() -> Result<()>`
- Dispatches to subcommand handlers via `match &cli.command`

**`cli::run::handle_run()`:**
- Location: `src/cli/run.rs`
- Triggers: `sqllog2db run` subcommand
- Responsibilities: File discovery, optional pre-scan, pipeline construction, sequential or parallel export loop, progress bar, resume state management

**`cli::stats::handle_stats()`:**
- Location: `src/cli/stats.rs`
- Triggers: `sqllog2db stats` subcommand
- Responsibilities: Stream all records, aggregate counts by time/user/app/ip buckets, print table or JSON

**`cli::digest::handle_digest()`:**
- Location: `src/cli/digest.rs`
- Triggers: `sqllog2db digest` subcommand
- Responsibilities: Stream records, compute SQL fingerprints via `features::fingerprint()`, aggregate by fingerprint, print ranked table

## Error Handling

**Strategy:** Non-fatal parse errors (malformed log lines) are logged as warnings and counted; processing continues. All other errors propagate via `Result<T>` and are converted to typed exit codes in `main()`.

**Exit codes:**
- `0` = success
- `2` = config error (`ConfigError`)
- `3` = IO / file / parser error
- `4` = export error (`ExportError`)
- `130` = user interrupt (Ctrl+C, `Error::Interrupted`)

**Error types** (`src/error.rs`):
- `Error::Config(ConfigError)` — TOML parse/validation failures, unknown keys, invalid values
- `Error::Parser(ParserError)` — path not found, invalid glob, dir read failure
- `Error::Export(ExportError)` — CSV write failure, SQLite operation failure
- `Error::File(FileError)` — generic file create/read/write errors
- `Error::Io(io::Error)` — low-level I/O
- `Error::Interrupted` — Ctrl+C signal

## Cross-Cutting Concerns

**Logging:** `log` facade with `env_logger`; for `run`/`stats`/`digest` commands, logs go to file only (configured via `[logging]`); other commands use console `env_logger`. Log rotation based on `retention_days`.

**Validation:** `Config::validate()` is always called before `handle_run()`; field-level errors include the config key path and invalid value in the error message.

**Interrupt handling:** `ctrlc` crate sets an `AtomicBool`; the hot loop checks it every 1024 records and breaks cleanly. `Error::Interrupted` propagates to `main()` for silent exit with code 130.

**Memory allocator:** `mimalloc` replaces the system allocator globally (`src/main.rs` `#[global_allocator]`).

---

*Architecture analysis: 2026-04-17*

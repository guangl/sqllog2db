# External Integrations

**Analysis Date:** 2026-04-17

## APIs & External Services

**GitHub Releases (self-update):**
- Service: GitHub REST API via `self_update` crate
- Purpose: Check for and download newer binary releases at startup and via `sqllog2db self-update`
- SDK/Client: `self_update` 0.44.0 (backend: `self_update::backends::github`)
- Auth: None (public repo); TLS via `rustls`
- Repo: `guangl/sqllog2db` (hardcoded in `src/cli/update.rs`)
- Triggered: on every non-quiet startup (version check) and explicitly via `self-update` subcommand

**crates.io Registry:**
- Used only at build time; `dm-database-parser-sqllog` is fetched from crates.io
- No runtime dependency on crates.io

## Data Storage

**Databases:**
- SQLite (local file)
  - Client: `rusqlite` 0.39.0 with bundled SQLite (no system library required)
  - Connection: path configured via `exporter.sqlite.database_url` in `config.toml` (default: `export/sqllog2db.db`)
  - Table: configurable via `exporter.sqlite.table_name` (default: `sqllog_records`)
  - Schema: 15 columns matching `Sqllog` record fields; INSERT is prepared once per run
  - Implementation: `src/exporter/sqlite.rs`

**File Storage:**
- Input: DaMeng SQL log files (`.log`); path/directory/glob configured via `sqllog.path` (default: `sqllogs/`)
- Output CSV: configurable via `exporter.csv.file` (default: `outputs/sqllog.csv`)
- Output log: application log file at `logging.file` (default: `logs/sqllog2db.log`)
- Resume state: `.sqllog2db_state.toml` (default) tracks processed-file digests for incremental runs
- All I/O is local filesystem; no remote object storage

**Caching:**
- None (stateless per run, except optional resume state file)

## Authentication & Identity

**Auth Provider:**
- None — tool operates entirely on local files; no user authentication
- GitHub API access for self-update is unauthenticated (public release endpoint)

## Monitoring & Observability

**Error Tracking:**
- None (no Sentry, Datadog, etc.)

**Logs:**
- File: configurable path (default `logs/sqllog2db.log`), rotation by retention_days (default 7)
- Console: `env_logger` for init/validate/update commands; progress bar via `indicatif` for run/stats/digest
- Structured: `thiserror`-derived error types with path/reason context; parse errors written to error log and processing continues
- Log levels: trace, debug, info, warn, error (configured via `logging.level`)

## CI/CD & Deployment

**Hosting:**
- GitHub Releases (binary distribution); referenced in `src/cli/update.rs` as `guangl/sqllog2db`

**CI Pipeline:**
- GitHub Actions: `workflows/ci.yaml` and `workflows/release.yaml` (present in repo, not in `.github/` but in root `workflows/` directory per local listing)

## Environment Configuration

**Required config (config.toml):**
- `[sqllog] path` — input log file path, directory, or glob
- `[exporter.csv] file` OR `[exporter.sqlite] database_url` — at least one exporter required

**Optional config:**
- `[logging] file`, `level`, `retention_days`
- `[features]` — filter/projection pipeline
- `[resume] state_file`

**Secrets location:**
- No secrets required; no API keys, credentials, or tokens needed at runtime

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None (self-update makes outbound HTTP GET to GitHub Releases API, but it is user-initiated or advisory-only at startup)

## External Parsing Dependency

**`dm-database-parser-sqllog` 0.9.1:**
- Source: crates.io (registry dependency, not a local path)
- Purpose: sole parser for DaMeng database SQL log format; exposes `Sqllog`, `MetaParts`, `PerformanceMetrics` types
- Used in: `src/exporter/csv.rs`, `src/exporter/sqlite.rs`, `src/cli/run.rs`, `src/features/`
- Risk: opaque external dependency; any breaking change in its API requires coordinated update

---

*Integration audit: 2026-04-17*

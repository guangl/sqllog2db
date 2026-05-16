---
phase: 14-exporter
plan: "04"
subsystem: cli
tags: [rust, cli, orchestration, template-stats, e2e]

# Dependency graph
requires:
  - phase: 14-exporter/14-01
    provides: ExporterManager::from_csv, write_template_stats public API
  - phase: 14-exporter/14-02
    provides: SqliteExporter::write_template_stats (creates sql_templates table)
  - phase: 14-exporter/14-03
    provides: CsvExporter::write_template_stats (writes companion _templates.csv)
provides:
  - handle_run sequential path calls write_template_stats(stats, None) after finalize()
  - handle_run parallel path constructs temporary ExporterManager::from_csv and calls write_template_stats(stats, Some(final_path))
  - test_no_template_stats_when_disabled (TMPL-04-D SC-4 coverage)
  - test_template_stats_enabled_end_to_end_sequential (TMPL-04-C SC-3 indirect e2e)
affects: [Phase 14 ROADMAP success criteria SC-1 through SC-4 all satisfied]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - conditional Some guard naturally prevents write_template_stats when disabled
    - parallel path uses throw-away ExporterManager::from_csv (no initialize/finalize needed)
    - D-02 compliant unique call point through ExporterManager trait

key-files:
  created: []
  modified:
    - src/cli/run.rs

key-decisions:
  - "Sequential path passes None as final_path; CsvExporter derives companion path from self.path (D-09)"
  - "Parallel path passes Some(Path::new(&csv_cfg.file)) satisfying D-03 explicit final_path"
  - "Temporary ExporterManager in parallel path never calls initialize() — write_template_stats does not depend on writer state"
  - "Both write_template_stats calls are strictly inside if-let Some guards, guaranteeing SC-4 (disabled = no file)"

patterns-established:
  - "Finalize-then-write-stats pattern: exporter_manager.finalize()? must precede write_template_stats (SC-3)"
  - "Parallel throw-away EM: ExporterManager::from_csv + write_template_stats without initialize/finalize lifecycle"

requirements-completed: [TMPL-04]

# Metrics
duration: 15min
completed: 2026-05-16
---

# Phase 14 Plan 04: CLI Orchestration End-to-End Wiring Summary

**write_template_stats() wired into both sequential and parallel run paths with 2 e2e integration tests covering enabled (SC-3) and disabled (SC-4) states**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-05-16
- **Completed:** 2026-05-16
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Sequential path (L900): `exporter_manager.write_template_stats(stats, None)?` inserted after `finalize()` and inside `if let Some(ref stats)` guard
- Parallel path (L793-L799): temporary `ExporterManager::from_csv` constructed; `tmp_em.write_template_stats(stats, Some(Path::new(&csv_cfg.file)))?` called inside `if let Some(ref stats)` guard
- `test_no_template_stats_when_disabled` (TMPL-04-D): asserts `out_templates.csv` absent when `template_analysis` not configured
- `test_template_stats_enabled_end_to_end_sequential` (TMPL-04-C): asserts main CSV non-empty + companion CSV present with correct 10-column header and ≥2 lines

## Task Commits

1. **Task 1: wire write_template_stats into sequential and parallel paths + 2 tests** - `a0c50df` (feat)

## Files Created/Modified

- `src/cli/run.rs` — 2 call site insertions (L900 sequential, L793-L799 parallel) + 2 new integration tests at module end

## Insertion Locations (Actual)

### Sequential path (顺序路径)

Line ~900 inside `if let Some(ref stats) = template_stats`:

```rust
exporter_manager.write_template_stats(stats, None)?;
```

Strictly after `exporter_manager.finalize()?` (L886) and `exporter_manager.log_stats()` (L887-889), satisfying SC-3 timing.

### Parallel path (并行路径)

Lines ~793-799 inside `if let Some(ref stats) = template_stats`:

```rust
if let Some(csv_cfg) = final_cfg.exporter.csv.as_ref() {
    let tmp_csv = CsvExporter::new(&csv_cfg.file);
    let mut tmp_em = ExporterManager::from_csv(tmp_csv);
    tmp_em.write_template_stats(stats, Some(Path::new(&csv_cfg.file)))?;
}
```

Temporary EM does not call `initialize()` — complies with D-02/D-03 constraints.

## Integration Tests Added

| Test Name | Coverage | Status |
|-----------|----------|--------|
| `test_no_template_stats_when_disabled` | TMPL-04-D / SC-4 | PASS |
| `test_template_stats_enabled_end_to_end_sequential` | TMPL-04-C / SC-3 (indirect) | PASS |

## Decisions Made

- Sequential path passes `None` as `final_path`; `CsvExporter` falls back to `self.path` for companion derivation (D-09)
- Parallel path passes `Some(Path::new(&csv_cfg.file))` to satisfy D-03 explicit final_path requirement
- Temporary `ExporterManager` in parallel path intentionally skips `initialize()`; `write_template_stats` implementation does not depend on writer being open
- Both insertion points inside `if let Some` guards — no additional `if enabled` guard needed (SC-4 satisfied by None propagation)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - clippy, fmt, and all 50 tests passed on first attempt.

## ROADMAP Success Criteria Status

| SC | Description | Status |
|----|-------------|--------|
| SC-1 | SQLite export generates sql_templates table (10 cols) | Satisfied by Plan 02 + sequential wiring |
| SC-2 | CSV export generates companion _templates.csv with consistent columns | Satisfied by Plan 03 + sequential/parallel wiring |
| SC-3 | write_template_stats called after main exporter finalize(); not called on interrupt | Satisfied structurally + e2e test indirect verification |
| SC-4 | disabled state: no sql_templates table / no companion file | Satisfied by if-let Some guard + test_no_template_stats_when_disabled |

## Next Phase Readiness

Phase 14 is complete. All 4 ROADMAP success criteria satisfied. `cargo run -- run -c config.toml` with `[features.template_analysis] enabled = true` will produce template statistics output in both CSV and SQLite modes.

---
*Phase: 14-exporter*
*Completed: 2026-05-16*

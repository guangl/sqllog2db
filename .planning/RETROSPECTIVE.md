# Retrospective

## Milestone: v1.0 — 增强 SQL 内容过滤与字段投影

**Shipped:** 2026-04-18
**Phases:** 2 | **Plans:** 6

### What Was Built

- Pre-compiled regex filter structs (`CompiledMetaFilters` + `CompiledSqlFilters`) with AND cross-field / OR intra-field semantics, startup validation
- `FilterProcessor` hot path integrated with compiled regex on all 7 meta fields
- `FeaturesConfig::ordered_field_indices()` for user-specified field order projection
- `CsvExporter` + `SqliteExporter` extended with `ordered_indices` — full field projection pipeline
- End-to-end wiring through `ExporterManager` and parallel CSV path

### What Worked

- **TDD RED/GREEN pattern** — writing failing tests first caught interface design issues early (Plan 01-01)
- **Pre-compile at startup** strategy — moving regex compilation to startup (not hot loop) kept the performance guarantee simple to reason about
- **`#[allow(dead_code)]` staging** — marking new structs as dead_code in Plan 01, removing in Plan 02 made the two-plan dependency explicit and clean
- **Atomic plan commits** — each plan had a clean, reviewable commit; deviations (clippy fixes) were folded in without scope creep

### What Was Inefficient

- REQUIREMENTS.md checkboxes were never updated during phase execution — required manual acknowledgement at milestone close
- STATE.md Performance Metrics section was left with placeholder dashes throughout the milestone (not auto-populated)

### Patterns Established

- `ordered_indices: Vec<usize>` as the projection API — cleaner than FieldMask bitmask for arbitrary ordering
- Reference-based construction (`FilterProcessor::new(&FiltersFeature)`) avoids clippy `needless_pass_by_value` from the start
- Re-export compiled types via `features::mod` for a clean public API boundary

### Key Lessons

- Clippy `-D warnings` catches interface design issues (pass-by-value, dead_code, must_use) that are cheaper to fix during the plan than after
- Two-plan structure (core structs → hot path wiring) worked well for regex feature: Plan 01 was pure logic, Plan 02 was pure integration — no mixing
- `ordered_indices` replacing FieldMask was the right call: the FieldMask approach would have required separate ordering metadata anyway

### Cost Observations

- Sessions: single-day execution (2026-04-18)
- Notable: all 6 plans executed sequentially in one session with no context resets required

---

## Cross-Milestone Trends

| Metric | v1.0 |
|--------|------|
| Phases | 2 |
| Plans | 6 |
| Days | 1 |
| Auto-fixed deviations | 6 (all clippy) |
| Scope creep | 0 |
| Test suite at close | 629+ |

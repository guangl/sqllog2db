---
phase: 06-parser-acceptance
reviewed: 2026-05-10T00:00:00Z
depth: standard
files_reviewed: 3
files_reviewed_list:
  - Cargo.toml
  - config.toml
  - benches/BENCHMARKS.md
findings:
  critical: 0
  warning: 3
  info: 3
  total: 6
status: issues_found
---

# Phase 6: Code Review Report

**Reviewed:** 2026-05-10
**Depth:** standard
**Files Reviewed:** 3
**Status:** issues_found

## Summary

Phase 6 made three changes: upgraded `dm-database-parser-sqllog` from 0.9.1 to 1.0.0 in `Cargo.toml`, simplified `config.toml` (no `batch_size` field, which correctly relies on the serde default of 10 000), and appended PERF-07 evaluation documentation to `benches/BENCHMARKS.md`.

The dependency upgrade resolves cleanly (`cargo check` passes, `Cargo.lock` records version 1.0.0 with a valid checksum). No Rust source changes were made. The following issues were found across the three files.

---

## Warnings

### WR-01: Stale JSONL Reference in `Cargo.toml` Package Description

**File:** `Cargo.toml:7`
**Issue:** The package `description` field reads `"高性能 CLI 工具：流式解析达梦数据库 SQL 日志并导出到 CSV/JSONL/SQLite"`. The JSONL exporter (`src/exporter/jsonl.rs`) was removed in an earlier phase (confirmed by CHANGELOG.md and the absence of any `jsonl` source file under `src/exporter/`). Publishing to crates.io with this description would falsely advertise a feature that does not exist, which can mislead users and tooling.
**Fix:**
```toml
description = "高性能 CLI 工具：流式解析达梦数据库 SQL 日志并导出到 CSV/SQLite"
```

---

### WR-02: Stale JSONL Reference in `config.toml` Priority Comment

**File:** `config.toml:65`
**Issue:** The comment reads `# 同时配置多个时，按优先级使用：csv > jsonl > sqlite`. JSONL is not a valid exporter section in the current codebase (`[exporter.jsonl]` is not parsed; `src/exporter/mod.rs` only handles `exporter.csv` and `exporter.sqlite`). A user who reads this comment and adds `[exporter.jsonl]` will get a config validation error with no useful explanation, or silent ignore.
**Fix:**
```toml
# 同时配置多个时，按优先级使用：csv > sqlite
```

---

### WR-03: Phase 5 Conclusion Records Only 50 Tests Instead of Full Suite

**File:** `benches/BENCHMARKS.md:281`
**Issue:** The Phase 5 conclusion states `全部 cargo test 通过（50 个）`. Actual `cargo test` currently yields 291 + 310 + 50 = 651 tests across three harnesses. The 50 count matches only the integration test binary (`tests/integration.rs`). Phase 4 correctly recorded 649 tests. Reporting 50/651 as "all tests passed" means the unit-test suites (which contain the bulk of correctness coverage) were not run — or the result was selectively reported — at Phase 5 close. This leaves an unverified quality gate in the benchmark log.
**Fix:** Correct the Phase 5 conclusion line to reflect the full suite:
```markdown
- [x] 全部 cargo test 通过（651 个），clippy/fmt 净化
```
and add a process note that `cargo test` must be run without `--test` filtering to capture all harnesses.

---

## Info

### IN-01: Placeholder Email in `Cargo.toml` Authors Field

**File:** `Cargo.toml:6`
**Issue:** `authors = ["guangl <guangl@example.com>"]` uses `example.com`, a reserved domain. While this does not affect compilation, if this crate is ever published to crates.io the metadata will be incorrect. The user's real email is `guangluo@outlook.com` (from project memory).
**Fix:**
```toml
authors = ["guangl <guangluo@outlook.com>"]
```

---

### IN-02: Hard-Limit Table Entry for `csv_export_real` Is Incorrect

**File:** `benches/BENCHMARKS.md:122`
**Issue:** The Performance rules table states the hard limit for `csv_export_real/real_file` is `≤ 0.347 s`. The v1.0 baseline median for that benchmark is recorded as `326.89 ms` (line 141), so the correct 5% ceiling is `326.89 × 1.05 = 343.23 ms ≈ 0.343 s`, not `0.347 s`. The discrepancy is 3.77 ms. While the limit is looser than intended (making it easier to pass), any automated regression gate built on this table will allow a 1.1% regression that should have been flagged.
**Fix:**
```markdown
| `csv_export_real/real_file`     | ≤ 0.343 s                       |
```

---

### IN-03: Phase 4 Table Shows Inconsistent Percentage for `csv_export/1000`

**File:** `benches/BENCHMARKS.md:139`
**Issue:** The summary table records `csv_export/1000` as `-3.42%` vs v1.0. However, the two values listed in the same row — baseline `239.16 µs` and Wave 2 result `238.04 µs` — imply only `-0.47%`. The `-3.42%` figure actually matches the Criterion output on line 155, which means Criterion's stored baseline JSON differs from the `239.16 µs` figure in the table. The table baseline value appears to have been entered manually and is inconsistent with the stored JSON, creating a misleading audit trail.
**Fix:** Either correct the baseline column to match what Criterion actually used (the value implied by `-3.42%` is approximately `246.1 µs`), or add a footnote clarifying that the "vs v1.0" column is taken from Criterion output rather than calculated from the adjacent columns.

---

_Reviewed: 2026-05-10_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

---
phase: 1
slug: zhengze-ziduan-guolv
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-18
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `cargo test` |
| **Config file** | `Cargo.toml` (harness = true, 默认) |
| **Quick run command** | `cargo test -p dm-database-sqllog2db filters` |
| **Full suite command** | `cargo test && cargo clippy --all-targets -- -D warnings` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p dm-database-sqllog2db filters`
- **After every plan wave:** Run `cargo test && cargo clippy --all-targets -- -D warnings`
- **Before `/gsd-verify-work`:** Full suite must be green + `cargo fmt --check`

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01 | 0 | FILTER-01 | — | N/A | unit | `cargo test -p dm-database-sqllog2db filters::tests::test_and_semantics_both_fields_required` | ❌ W0 | ⬜ pending |
| 1-01-02 | 01 | 0 | FILTER-01 | — | N/A | unit | `cargo test -p dm-database-sqllog2db filters::tests::test_regex_pattern_match` | ❌ W0 | ⬜ pending |
| 1-01-03 | 01 | 0 | FILTER-01 | — | N/A | unit | `cargo test -p dm-database-sqllog2db config::tests::test_invalid_regex_returns_error` | ❌ W0 | ⬜ pending |
| 1-01-04 | 01 | 0 | FILTER-02 | — | N/A | unit | `cargo test -p dm-database-sqllog2db filters::tests::test_record_sql_regex_include` | ❌ W0 | ⬜ pending |
| 1-01-05 | 01 | 0 | FILTER-02 | — | N/A | unit | `cargo test -p dm-database-sqllog2db filters::tests::test_record_sql_regex_exclude` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/features/filters.rs` (tests 模块) — 新增 AND 语义 + 正则匹配测试用例（文件已存在）
- [ ] `src/config.rs` (tests 模块) — 新增非法正则验证测试用例（文件已存在）

*现有 test infrastructure 已就绪，Wave 0 仅需新增测试用例。*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

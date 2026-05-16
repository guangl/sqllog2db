---
phase: 12
slug: sql
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-15
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test（cargo test） |
| **Config file** | 无独立配置文件 |
| **Quick run command** | `cargo test -p dm-database-sqllog2db` |
| **Full suite command** | `cargo clippy --all-targets -- -D warnings && cargo test` |
| **Estimated runtime** | ~2 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p dm-database-sqllog2db`
- **After every plan wave:** Run `cargo clippy --all-targets -- -D warnings && cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 12-01-01 | 01 | 1 | TMPL-01 | — | N/A | unit | `cargo test template_analysis_config` | ❌ Wave 0 | ⬜ pending |
| 12-01-02 | 01 | 1 | TMPL-01 | — | N/A | unit | `cargo test normalize_template` | ❌ Wave 0 | ⬜ pending |
| 12-01-03 | 01 | 1 | TMPL-01 | — | N/A | unit | `cargo test normalize_template::comment` | ❌ Wave 0 | ⬜ pending |
| 12-01-04 | 01 | 1 | TMPL-01 | — | N/A | unit | `cargo test normalize_template::in_fold` | ❌ Wave 0 | ⬜ pending |
| 12-01-05 | 01 | 1 | TMPL-01 | — | N/A | unit | `cargo test normalize_template::keyword_case` | ❌ Wave 0 | ⬜ pending |
| 12-01-06 | 01 | 1 | TMPL-01 | — | N/A | unit | `cargo test normalize_template::whitespace` | ❌ Wave 0 | ⬜ pending |
| 12-01-07 | 01 | 1 | TMPL-01 | — | N/A | unit | `cargo test normalize_template::literal_protection` | ❌ Wave 0 | ⬜ pending |
| 12-02-01 | 02 | 2 | TMPL-01 | — | N/A | lint | `cargo clippy --all-targets -- -D warnings` | ✅ 已有 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

**TDD 内联模式**：测试与实现在 Plan 01 Task 2 同一 task 内编写（`tdd="true"`），语义等价于 Wave 0 预建立测试骨架。Plan 01 Task 2 要求先编写测试函数骨架再实现，满足 Nyquist 反馈循环要求。

- [x] `src/features/sql_fingerprint.rs` — `normalize_template()` 测试模块随 Task 2 一同创建（TDD 内联）
- [x] `src/features/mod.rs` — `TemplateAnalysisConfig` 单元测试随 Plan 02 Task 1 一同创建

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

---
phase: 13
slug: templateaggregator
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-16
---

# Phase 13 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test template` |
| **Full suite command** | `cargo test && cargo clippy --all-targets -- -D warnings` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test template`
- **After every plan wave:** Run `cargo test && cargo clippy --all-targets -- -D warnings`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 13-01-01 | 01 | 1 | TMPL-02 | — | N/A | unit | `cargo test template_aggregator` | ❌ W0 | ⬜ pending |
| 13-01-02 | 01 | 1 | TMPL-02 | — | N/A | unit | `cargo test template_stats` | ❌ W0 | ⬜ pending |
| 13-01-03 | 01 | 1 | TMPL-02 | — | N/A | unit | `cargo test observe_finalize_merge` | ❌ W0 | ⬜ pending |
| 13-02-01 | 02 | 2 | TMPL-02 | — | N/A | integration | `cargo test && cargo clippy --all-targets -- -D warnings` | ✅ | ⬜ pending |
| 13-02-02 | 02 | 2 | TMPL-02 | — | N/A | integration | `cargo build --release` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/features/template_aggregator.rs` — new file with TemplateEntry, TemplateAggregator, TemplateStats structs

*Existing Rust test infrastructure covers all other phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| 禁用统计时热循环行为与 v1.2 完全一致 | TMPL-02 | 性能基准对比 | `cargo bench` 对比有无 aggregator 的吞吐量 |
| parallel CSV merge 结果与单线程一致 | TMPL-02 | 需要多线程 + 真实日志文件 | 分别运行 CSV 和 SQLite 导出，比对 finalize() 输出 |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

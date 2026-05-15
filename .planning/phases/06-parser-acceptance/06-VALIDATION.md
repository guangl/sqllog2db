---
phase: 6
slug: parser-acceptance
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-15
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust 内置) + cargo clippy + cargo fmt |
| **Config file** | `Cargo.toml` |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check` |
| **Estimated runtime** | ~30 seconds (test) + ~5 seconds (clippy/fmt) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test && cargo clippy --all-targets -- -D warnings`
- **Before `/gsd-verify-work`:** 三项验收命令全部通过
- **Max feedback latency:** ~30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 6-01-01 | 01 | 1 | PERF-07 | — | N/A | documentation | `grep -c "PERF-07" benches/BENCHMARKS.md` | ✅ | ✅ green |
| 6-01-02 | 01 | 1 | PERF-07 | — | N/A | build | `grep "dm-database-parser-sqllog.*1\.0\.0" Cargo.toml` | ✅ | ✅ green |
| 6-02-01 | 02 | 2 | PERF-09 | — | N/A | unit+integration | `cargo test` | ✅ | ✅ green |
| 6-02-02 | 02 | 2 | PERF-09 | — | N/A | lint | `cargo clippy --all-targets -- -D warnings` | ✅ | ✅ green |
| 6-02-03 | 02 | 2 | PERF-09 | — | N/A | format | `cargo fmt --check` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] Cargo.toml 中 dm-database-parser-sqllog 升级至 1.0.0（PERF-07 调研结论确认 0.9.1 → 1.0.0 改进自动生效，见 06-01-SUMMARY.md commit 4654846）

*现有 criterion + cargo test/clippy/fmt 基础设施覆盖 Phase 6 所有验收要求，无新增基础设施需求。*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| PERF-07 调研结论的合理性 | PERF-07 | 调研结论涉及 API 设计取舍（index()/RecordIndex 是否集成），需人工判断流式场景适用性 | 阅读 benches/BENCHMARKS.md "Phase 6 — 解析库集成评估（PERF-07）" 段落，确认 mmap/par_iter()/编码检测/MADV_SEQUENTIAL 自动生效 + index() 不集成理由清晰 |

> 依据 06-VERIFICATION.md，PERF-07 调研结论已被用户认可（human_verification 块标记 result: pass）。

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** signed retroactively 2026-05-15（执行于 2026-05-10，VALIDATION.md 补签于 Phase 11，DEBT-03）。验收依据：06-VERIFICATION.md 3/3 truths verified（cargo test 651 passed / clippy 0 warnings / fmt 0 diff）；Plan 01 PERF-07 调研结论已记录于 benches/BENCHMARKS.md；Plan 02 v1.1 milestone 三项验收门控全部通过。

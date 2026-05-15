---
phase: 3
slug: profiling-benchmarking
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-26
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo criterion |
| **Config file** | Cargo.toml (bench configuration) |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test && CRITERION_HOME=benches/baselines cargo bench` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test && CRITERION_HOME=benches/baselines cargo bench`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 3-01-01 | 01 | 1 | PERF-01 | — | N/A | benchmark | `CRITERION_HOME=benches/baselines cargo bench` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `[profile.flamegraph]` section in `Cargo.toml` with `debug = true` + `strip = "none"` — required before flamegraph tasks

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| flamegraph 生成有符号调用链 | PERF-01 | 需人工检查 SVG 输出中的函数名可读性 | 运行 `cargo flamegraph --profile flamegraph --bin sqllog2db -- run -c config.toml`，确认输出 SVG 中函数名非 `[unknown]` |
| baseline 基准数据与 v1.0 吞吐对齐 | PERF-01 | 需人工对比数值与 README 中记录值 | 运行 benchmark 后检查 `benches/baselines/` JSON，确认 CSV real ~1.55M rec/s |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** signed 2026-05-15 — 回溯补签于 Phase 11（DEBT-03）；执行验证于 2026-04-27（见 03-VERIFICATION.md，10/10 truths verified）。

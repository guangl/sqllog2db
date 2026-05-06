---
phase: 4
slug: csv
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-27
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | criterion 0.7 + `cargo test` (rust built-in) |
| **Config file** | `Cargo.toml` `[[bench]]` + `[dev-dependencies]` |
| **Quick run command** | `cargo test --lib -- exporter::csv` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~30 seconds (test) + ~3 min (benchmarks) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib -- exporter::csv`
- **After every plan wave:** Run `cargo test` + `CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0`
- **Before `/gsd-verify-work`:** Full suite must be green + criterion 对比报告显示 ≥10% 提升
- **Max feedback latency:** ~30 seconds (unit tests)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 4-01-01 | 01 | 0 | PERF-03 | — | N/A | benchmark | `cargo bench --bench bench_csv -- csv_format_only` | ❌ W0 新增 | ⬜ pending |
| 4-01-02 | 01 | 0 | PERF-03 | — | N/A | unit | `cargo test --lib -- exporter::csv` | ✅ | ⬜ pending |
| 4-02-01 | 02 | 1 | PERF-03 | — | N/A | benchmark | `cargo bench --bench bench_csv -- csv_format_only` | ❌ W0 | ⬜ pending |
| 4-02-02 | 02 | 1 | PERF-08 | — | N/A | unit | `cargo test --lib -- exporter::csv` | ✅ | ⬜ pending |
| 4-03-01 | 03 | 2 | PERF-05/D-05 | — | N/A | unit | `cargo test --lib` | ✅ | ⬜ pending |
| 4-04-01 | 04 | 2 | PERF-02 | — | N/A | benchmark | `CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0` | ✅ | ⬜ pending |
| 4-04-02 | 04 | 2 | 无回归 | — | N/A | unit | `cargo test` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `benches/bench_csv.rs` 新增 `bench_csv_format_only` group — 覆盖 PERF-03（格式化路径隔离）
- [ ] `src/exporter/csv.rs` 中 `write_record_preparsed` 改为 `pub(crate)` — 使 benchmark 可直接调用

*现有 criterion + cargo test 基础设施覆盖其余 Phase 要求。*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| 热循环堆分配减少（flamegraph diff） | PERF-08 | flamegraph 需视觉比对，无法自动化 | `samply record --save-only --output docs/flamegraphs/csv_export_real_phase4.json -- cargo bench --profile flamegraph --bench bench_csv -- --profile-time 15 csv_export_real/real_file`，与 Phase 3 flamegraph 对比 parse_meta 和 _platform_memmove 比例 |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

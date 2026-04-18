---
phase: 2
slug: shuchu-ziduan-kongzhi
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-18
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test (`#[test]`) |
| **Config file** | 无独立配置，`cargo test` 即可 |
| **Quick run command** | `cargo test -q 2>&1 \| tail -5` |
| **Full suite command** | `cargo test && cargo clippy --all-targets -- -D warnings` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -q 2>&1 | tail -5`
- **After every plan wave:** Run `cargo test && cargo clippy --all-targets -- -D warnings`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** ~10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 2-01-01 | 01 | 1 | FIELD-01 | — | N/A | unit | `cargo test test_ordered_field_indices` | ❌ Wave 0 | ⬜ pending |
| 2-01-02 | 01 | 1 | FIELD-01 | — | N/A | unit | `cargo test test_csv_field_order` | ❌ Wave 0 | ⬜ pending |
| 2-01-03 | 01 | 1 | FIELD-01 | — | N/A | unit | `cargo test test_sqlite_field_order` | ❌ Wave 0 | ⬜ pending |
| 2-01-04 | 01 | 2 | FIELD-01 | — | N/A | integration | `cargo test test_parallel_csv_field_order` | ❌ Wave 0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/features/mod.rs` — 新增 `test_ordered_field_indices` 单元测试（含 None/空列表/正常配置/边界 normalized_sql）
- [ ] `src/exporter/csv.rs` — 新增 `test_csv_field_order` 和 `test_csv_field_order_partial` 单元测试
- [ ] `src/exporter/sqlite.rs` — 新增 `test_sqlite_field_order` 和 `test_sqlite_field_order_partial` 单元测试
- [ ] `src/cli/run.rs` — 新增 `test_parallel_csv_field_order` 集成测试（并行路径字段顺序验证）

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| 端到端 CSV 输出列顺序目视检查 | FIELD-01 | 需要真实 log 文件 | `cargo run -- run -c config.toml`，打开 output.csv 确认列顺序与 fields 配置一致 |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

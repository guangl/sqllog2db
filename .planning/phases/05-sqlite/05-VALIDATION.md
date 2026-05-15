---
phase: 5
slug: sqlite
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-09
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + criterion 0.7 |
| **Config file** | `Cargo.toml` `[[bench]]` 条目 |
| **Quick run command** | `cargo test --lib -- exporter::sqlite` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~30 seconds (full suite) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib -- exporter::sqlite`
- **After every plan wave:** Run `cargo test` + `cargo bench --bench bench_sqlite -- --sample-size 10`
- **Before `/gsd-verify-work`:** Full suite must be green + baseline comparison passing
- **Max feedback latency:** ~30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 5-01-01 | 01 | 1 | PERF-04/PERF-06 | — | N/A | unit | `cargo test --lib -- config` | ❌ W0 | ✅ green |
| 5-01-02 | 01 | 1 | PERF-04 | — | N/A | build | `cargo build --bench bench_sqlite` | ❌ W0 | ✅ green |
| 5-02-01 | 02 | 2 | PERF-05 | — | N/A | integration (N/A — WAL removed) | N/A — `cargo test --lib -- exporter::sqlite::tests::test_sqlite_wal_mode_enabled` | ❌ W0 | N/A |
| 5-02-02 | 02 | 2 | PERF-04/PERF-06 | — | N/A | integration | `cargo test --lib -- exporter::sqlite` | ❌ W0 | ✅ green |
| 5-03-01 | 03 | 3 | PERF-06 | — | N/A | code review | 代码审查 + flamegraph 确认无重复 prepare() | ✅ (samply installed) | ✅ green |
| 无回归 | all | all | — | — | N/A | unit+integration | `cargo test` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [N/A] `src/exporter/sqlite.rs` — WAL 测试：`test_sqlite_wal_mode_enabled`（断言 journal_mode 返回 "wal"，PERF-05）  *N/A — 用户决策移除（PERF-05 canceled）：数据无需崩溃保护，保留 JOURNAL_MODE=OFF SYNCHRONOUS=OFF 高性能模式。见 05-VERIFICATION.md §Re-verification Context。*
- [N/A] `src/exporter/sqlite.rs` — page_size 测试：`test_sqlite_wal_page_size`（断言 WAL 下 page_size=65536 生效）  *N/A — 用户决策移除（PERF-05 canceled）：WAL 模式整体取消，page_size 测试无意义。*
- [x] `src/exporter/sqlite.rs` — 批量提交测试：`test_sqlite_batch_commit`（用 batch_size=2 写 5 条记录，验证中间 COMMIT 路径，PERF-04）
- [x] `benches/bench_sqlite.rs` — 新增 `sqlite_single_row` benchmark group（PERF-04 对照基线）
- [x] `src/config.rs` — `SqliteExporter` struct 新增 `batch_size: usize` 字段（含 serde default = 10000）

*如无现有 infrastructure 缺失：上述为 Phase 5 专属新增项，其余 649 个测试已覆盖基础功能。*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| flamegraph 确认无重复 sqlite3_prepare_v3 | PERF-06 | flamegraph 采集需手动解读 | `samply record cargo bench --profile flamegraph --bench bench_sqlite -- --profile-time 10`，确认热循环中无 `sqlite3_prepare_v3` 采样 |
| BENCHMARKS.md 数值更新 | PERF-04 | 需人工对比 criterion baseline 输出 | `CRITERION_HOME=benches/baselines cargo bench --bench bench_sqlite -- --baseline v1.0`，将输出写入 `benches/BENCHMARKS.md` |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** signed 2026-05-15 — 回溯补签于 Phase 11（DEBT-03）；执行验证于 2026-05-10（见 05-VERIFICATION.md，4/4 truths verified）。WAL 相关项（任务 5-02-01、Wave 0 两条 WAL 测试）依 D-03 标记 N/A：PERF-05 用户决策移除，保留 JOURNAL_MODE=OFF SYNCHRONOUS=OFF 高性能模式。

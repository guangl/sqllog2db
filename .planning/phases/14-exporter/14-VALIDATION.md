---
phase: 14
slug: exporter
status: compliant
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-16
audited: 2026-05-17
---

# Phase 14 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test && cargo clippy --all-targets -- -D warnings` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test && cargo clippy --all-targets -- -D warnings`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

每个计划目前只含 1 个核心任务，故 Task ID 命名为 `14-<plan>-01`。Test 列出该任务直接覆盖的测试函数（与 RESEARCH §"Phase Requirements → Test Map" 一一对应）。

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 14-01-01 | 01 | 1 | TMPL-04 | T-14-01 / T-14-02 | trait 默认 no-op + DryRun info! 不写文件 | unit | `cargo test --lib exporter::tests::test_default_write_template_stats_noop test_dry_run_write_template_stats_noop test_exporter_manager_write_template_stats_dry_run test_exporter_kind_dispatch_write_template_stats` | ✅ | ✅ green |
| 14-02-01 | 02 | 2 | TMPL-04 (A/E/F) | T-14-03 / T-14-04 / T-14-05 | DDL 字面量 + params! 参数化绑定 | integration | `cargo test --lib exporter::sqlite test_sqlite_write_template_stats test_sqlite_templates_overwrite test_sqlite_templates_append` | ✅ | ✅ green |
| 14-03-01 | 03 | 2 | TMPL-04 (B/H) | T-14-06 / T-14-07 / T-14-08 | write_csv_escaped + File::create 覆盖 + flush | integration | `cargo test --lib exporter::csv test_csv_write_template_stats test_parallel_csv_companion_file` | ✅ | ✅ green |
| 14-04-01 | 04 | 3 | TMPL-04 (C/D) | T-14-09 / T-14-10 / T-14-11 | finalize-后调用 + disabled-state 自动跳过 + 临时 ExporterManager 仅持轻量结构 | integration | `cargo test --lib cli::run test_no_template_stats_when_disabled test_template_stats_enabled_end_to_end_sequential` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

### Plan ↔ TMPL-04 Sub-requirement Coverage

| Sub-Req | Test | Owning Plan | Status |
|---------|------|-------------|--------|
| TMPL-04-A SQLite 表存在含 10 列 | `test_sqlite_write_template_stats` | 02 | ✅ |
| TMPL-04-B CSV 伴随文件含表头+行 | `test_csv_write_template_stats` | 03 | ✅ |
| TMPL-04-C finalize 后调用（顺序路径时序） | `test_template_stats_enabled_end_to_end_sequential`（结构+e2e） | 04 | ✅ |
| TMPL-04-D enabled=false 不创建表/伴随 | `test_no_template_stats_when_disabled` | 04 | ✅ |
| TMPL-04-E overwrite=true 重建 sql_templates | `test_sqlite_templates_overwrite` | 02 | ✅ |
| TMPL-04-F append=true 累加 sql_templates | `test_sqlite_templates_append` | 02 | ✅ |
| TMPL-04-G DryRun no-op | `test_dry_run_write_template_stats_noop` | 01 | ✅ |
| TMPL-04-H 并行路径生成伴随文件（final_path=Some） | `test_parallel_csv_companion_file` | 03 | ✅ |

---

## Wave 0 Requirements

- [x] `src/features/mod.rs` — `pub use template_aggregator::TemplateStats;` re-export（Plan 01）
- [x] `src/exporter/mod.rs` — `write_template_stats()` trait method + 默认实现 + ExporterKind 透传 + ExporterManager 公共方法 + DryRunExporter 覆盖 stub（Plan 01）
- [x] `src/exporter/sqlite.rs` — `SqliteExporter::write_template_stats()` 实现 + `create_or_replace_template_table()` 辅助（Plan 02）
- [x] `src/exporter/csv.rs` — `CsvExporter::write_template_stats()` + `build_companion_path()` + `write_companion_rows()` 三层（Plan 03）
- [x] `src/cli/run.rs` — 顺序路径插入 + 并行路径临时 ExporterManager 构造 + disabled-state 集成测试 + enabled-state 顺序 e2e 测试（Plan 04）

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| 主导出中断时伴随文件不被写出 | TMPL-04 SC-3（运行时中断） | 需要模拟 Ctrl+C 中断场景 | 运行 run 并在 finalize 前 Ctrl+C，确认无 sql_templates 表或 _templates.csv |

> 注：SC-3 中"write_template_stats 在 finalize 之后调用"的结构性保证由 Plan 04 顺序路径 e2e 测试 `test_template_stats_enabled_end_to_end_sequential` 间接验证；运行时中断场景仍保留为手工验证。

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** COMPLIANT — 2026-05-17

---

## Validation Audit 2026-05-17

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 4 tasks updated pending → green; 8 sub-requirements confirmed |
| Escalated | 0 |

**Audit notes:** Phase 14 was complete at audit time (all 4 SUMMARY.md present). All 8 TMPL-04 sub-requirement tests confirmed green: trait no-op, DryRun, SQLite overwrite/append, CSV companion, disabled-state, enabled e2e. Wave 0 requirements (5 files) all exist with required implementations. `cargo clippy --all-targets -- -D warnings` and `cargo test` both pass (418 tests). VALIDATION.md updated from draft to compliant.

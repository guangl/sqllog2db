---
phase: 16
slug: remaining-charts
status: compliant
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-17
audited: 2026-05-17
---

# Phase 16 — Validation Strategy

> Per-phase validation contract. Reconstructed from SUMMARY.md artifacts (no prior VALIDATION.md).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib charts` |
| **Full suite command** | `cargo test && cargo clippy --all-targets -- -D warnings` |
| **Estimated runtime** | ~15 seconds |

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 16-01-01 | 01 | 1 | CHART-04/05 | unit | `cargo test --lib test_iter_hour_counts_basic test_iter_user_counts_basic test_merge_hour_user_counts` | ✅ green |
| 16-02-01 | 02 | 1 | CHART-04/05 | unit | `cargo test --lib test_charts_config_deserialize_trend_user_flags test_charts_config_new_fields_default_true` | ✅ green |
| 16-03-01 | 03 | 2 | CHART-04 | unit | `cargo test --lib test_draw_trend_line_empty_returns_ok test_draw_trend_line_multi_hour test_build_x_labels_single_day test_build_x_labels_multi_day test_is_multi_day_true` | ✅ green |
| 16-04-01 | 04 | 2 | CHART-05 | unit | `cargo test --lib test_draw_user_pie_empty test_draw_user_pie_multiple_users test_prepare_slices_others_aggregation test_hsl_to_rgb_red_hue` | ✅ green |
| 16-05-01 | 05 | 3 | CHART-04/05 | integration | `cargo test && cargo clippy --all-targets -- -D warnings` | ✅ green |

---

## Wave 0 Requirements

- [x] `src/charts/trend_line.rs` — draw_trend_line() (Plan 03)
- [x] `src/charts/user_pie.rs` — draw_user_pie() (Plan 04)
- [x] `src/features/template_aggregator.rs` — iter_hour_counts() + iter_user_counts() (Plan 01)
- [x] `src/features/mod.rs` — trend_line/user_pie flags in ChartsConfig (Plan 02)
- [x] `src/charts/mod.rs` — trend_line/user_pie dispatch in generate_charts() (Plan 05)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| frequency_trend.svg 时间轴标签可读 | CHART-04 SC-1 | 视觉验收 | 打开 frequency_trend.svg 检查 X 轴 HH:00 或 MM-DD HH:00 格式 |
| user_schema_pie.svg 扇区颜色可区分 | CHART-05 SC-2 | 视觉验收 | 打开 user_schema_pie.svg 确认颜色对比度和标签截断 |

---

## Validation Sign-Off

- [x] All tasks have automated verify
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** COMPLIANT — 2026-05-17 (16-REVIEW.md status: fixed; 418 tests pass; clippy clean; draw_trend_line 7 tests + draw_user_pie 8 tests all green)

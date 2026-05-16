---
phase: 15
slug: svg
status: compliant
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-17
audited: 2026-05-17
---

# Phase 15 — Validation Strategy

> Per-phase validation contract. Reconstructed from SUMMARY.md + VERIFICATION.md artifacts.

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
| 15-01-01 | 01 | 1 | CHART-01 | unit | `cargo test --lib test_charts_config_default_values test_charts_config_deserialize_only_output_dir test_validate_charts_requires_template_analysis test_apply_one_charts_output_dir` | ✅ green |
| 15-01-02 | 01 | 1 | CHART-01 | unit | `cargo test --lib test_validate_charts_top_n_zero_is_rejected test_validate_charts_empty_output_dir_is_rejected test_apply_one_charts_frequency_bar_false` | ✅ green |
| 15-02-01 | 02 | 1 | CHART-02/03 | unit | `cargo test --lib test_iter_chart_entries_sort_order test_iter_chart_entries_single_key test_iter_chart_entries_empty` | ✅ green |
| 15-03-01 | 03 | 2 | CHART-02 | unit | `cargo test --lib test_sanitize_filename_ascii_alphanumeric test_sanitize_filename_truncate_80 test_draw_frequency_bar_creates_nonempty_svg test_truncate_label_exact_40` | ✅ green |
| 15-04-01 | 04 | 2 | CHART-03 | unit | `cargo test --lib test_draw_latency_hist_creates_nonempty_svg test_draw_latency_hist_empty_histogram test_extract_buckets_min_val_max_one test_draw_latency_hist_single_bucket` | ✅ green |
| 15-05-01 | 05 | 3 | CHART-01/02/03 | integration | `cargo test && cargo clippy --all-targets -- -D warnings` | ✅ green |

---

## Wave 0 Requirements

- [x] `src/charts/mod.rs` — generate_charts() entry point (Plan 03)
- [x] `src/charts/frequency_bar.rs` — draw_frequency_bar() (Plan 03)
- [x] `src/charts/latency_hist.rs` — draw_latency_hist() (Plan 04)
- [x] `src/features/mod.rs` — ChartsConfig + ChartEntry pub use (Plan 01/02)
- [x] `src/cli/run.rs` — generate_charts() dispatch in both paths (Plan 05)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| SVG 文件可被浏览器渲染（图表元素完整） | CHART-01 SC-1 | 需要浏览器打开 | 运行 `cargo run -- run -c config_with_charts.toml`，打开 charts/*.svg |
| Top N 条形图 Y 轴标签截断超长 fingerprint | CHART-02 SC-2 | 视觉验收 | 检查 top_n_frequency.svg 超 40 字符 key 是否截断并加 "…" |

---

## Validation Sign-Off

- [x] All tasks have automated verify
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** COMPLIANT — 2026-05-17 (reconstructed from Phase 15 VERIFICATION.md + SUMMARY.md; Wave 1 formally verified 9/9, Wave 2/3 confirmed via unit tests; 418 tests pass)

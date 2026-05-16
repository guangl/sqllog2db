---
plan: 12-03
phase: 12-sql
status: completed
commit: 344df9f
---

# Plan 12-03: normalize_template 接入热循环

## 完成内容

- `process_log_file` 签名新增 `do_template: bool` 参数（位于 `do_normalize` 之后）
- `process_csv_parallel` 签名同步新增 `do_template: bool`，并透传至内部 `process_log_file` 调用
- `handle_run()` 新增 `let do_template = final_cfg.features.template_analysis.as_ref().is_some_and(|t| t.enabled);`
- 热循环中 `let ns` 之后新增：`let _tmpl_key: Option<String> = if do_template { Some(crate::features::normalize_template(pm.sql.as_ref())) } else { None };`（D-13/D-14）
- 两处 `process_log_file` 调用点（并行 + 顺序）均传入 `do_template`

## 设计决策

- `_tmpl_key` 以 `_` 前缀命名，避免 Phase 13 接入前的 unused warning
- 禁用时（`do_template = false`）分支不进入，零分配开销（零开销快路径）
- Phase 13 的 `TemplateAggregator::observe()` 将消费 `_tmpl_key`（D-14）

## 验收通过

- `cargo clippy --all-targets -- -D warnings` 零 warning
- `cargo test` 全套 50 项通过（含已有集成测试零回归）
- Phase 12 三个 Plan 全部提交完成

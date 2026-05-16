---
phase: 13-templateaggregator
plan: "02"
status: complete
subsystem: cli/run
tags: [template-aggregator, logging, integration-tests, run-wiring]
dependency_graph:
  requires: ["13-01"]
  provides:
    - "process_log_file 侧路径接入（aggregator 参数已在 01 连通）"
    - "process_csv_parallel map-reduce（aggregator 已在 01 连通）"
    - "handle_run 顺序路径 finalize + info! 日志"
    - "handle_run 并行路径 finalize + info! 日志"
    - "Template analysis info! 日志（两路径均覆盖）"
    - "test_aggregator_disabled_none_path 集成测试"
    - "test_parallel_merge_consistent 集成测试"
  affects: ["src/cli/run.rs"]
tech_stack:
  added: []
  patterns:
    - "Option<Vec<TemplateStats>> 生命周期收尾：finalize() 在路径末尾调用，结果暂存供 Phase 14 消费"
    - "info! macro 用于结构化运行摘要输出"
key_files:
  modified: ["src/cli/run.rs"]
decisions:
  - "finalize 调用位置在两路径各自结束处（并行路径在断点续传写入前，顺序路径在 exporter_manager.finalize() 后）"
  - "aggregator 仅统计通过过滤且有 tag 的 DML 记录（observe 在 process_log_file 热循环中调用，由 01 完成）"
  - "exectime_us > 0 guard 确保 hdrhistogram 不接收零值（由 01 实现）"
  - "test_parallel_merge_consistent 验证顺序与并行路径输出行数一致，而非比较 TemplateStats 内容（避免并发写入顺序差异）"
metrics:
  duration: "~8 minutes"
  completed: "2026-05-16"
  tasks_completed: 3
  files_modified: 1
  tests_added: 2
---

# Phase 13 Plan 02: run.rs TemplateAggregator 接入收尾 Summary

**One-liner:** 在 run.rs 并行/顺序两路径末尾添加 `info!("Template analysis: N unique templates")` 日志，并补充 2 个集成测试覆盖 disabled 路径与并行一致性。

## What Was Built

### Task 1: info! 日志（两路径）

并行路径（约第 791 行）和顺序路径（约第 888 行）各自将原本的 `let _template_stats = ...` 替换为：

```rust
let template_stats = <agg>.map(TemplateAggregator::finalize);
if let Some(ref stats) = template_stats {
    info!("Template analysis: {} unique templates", stats.len());
}
```

两路径在 `do_template=false` 时 `template_stats` 为 `None`，`if let` 不执行，无性能开销。

### Task 2: 集成测试

**test_aggregator_disabled_none_path**
- 创建无 `[features.template_analysis]` 的最小配置
- 调用 `handle_run`（jobs=1），断言返回 `Ok`
- 验证 `do_template=false` 路径不 panic、不调用 finalize

**test_parallel_merge_consistent**
- 写入两个日志文件（满足并行路径前提：`log_files.len() > 1`）
- 分别以 jobs=1（顺序）和 jobs=4（并行）各运行一次，均启用 `template_analysis`
- 断言两次运行均 `Ok`，且输出 CSV 行数相同

## Deviations from Plan

**[Rule 1 - Bug] clippy doc_markdown: 注释中标识符未加 backtick**

- **Found during:** Task 3（cargo clippy -D warnings）
- **Issue:** `/// 当 features.template_analysis 未配置时，do_template=false，handle_run 应正常完成` 中的标识符触发 `clippy::doc_markdown`
- **Fix:** 改为 `` `features.template_analysis` ``、`` `do_template=false` ``、`` `handle_run` ``
- **Files modified:** `src/cli/run.rs`

**[Rule 1 - Bug] cargo fmt: 长行需换行格式化**

- **Found during:** Task 3（pre-commit hook 触发 cargo fmt check）
- **Issue:** `assert!(result.is_ok(), "...")` 与两处 `.join(...).to_string_lossy().replace(...)` 链超出行宽
- **Fix:** cargo fmt 自动换行，重新暂存提交
- **Files modified:** `src/cli/run.rs`

## Test Results

```
cargo test: 50 passed; 0 failed (run.rs 模块)
全项目: 349 passed; 0 failed
cargo clippy --all-targets -- -D warnings: clean
cargo build --release: ok (30.49s)
```

## Self-Check: PASSED

- [x] `src/cli/run.rs` 存在且包含 `info!("Template analysis:` 字符串（两处）
- [x] commit `ecc1d90` 存在
- [x] `test_aggregator_disabled_none_path` 和 `test_parallel_merge_consistent` 均在 349 个通过测试中

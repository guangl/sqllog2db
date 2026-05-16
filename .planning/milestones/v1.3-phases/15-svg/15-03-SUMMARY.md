---
plan: 15-03
status: complete
wave: 2
---

# Plan 15-03 Summary

## What Was Done

为 Phase 15 SVG 图表生成实现核心基础设施：plotters 依赖、charts 模块入口、频率柱状图生成器。

1. **Task 1 - Cargo.toml + src/charts/mod.rs**
   - 在 `Cargo.toml` 新增 `plotters = 0.3.7`（`svg_backend` + `all_series` + `all_elements`，default-features = false）
   - 在 `src/lib.rs` 注册 `pub mod charts`
   - 新建 `src/charts/mod.rs`：
     - `pub fn generate_charts(agg, cfg)` 入口函数，按 `cfg.frequency_bar` / `cfg.latency_hist` 开关分发
     - `draw_all_latency_hists()` 私有辅助（迭代 top_n 条目，生成命名文件）
     - `sanitize_filename()` 将非 ASCII/非字母数字字符替换为 `_`，截断到 80 字符
     - 5 个 `sanitize_filename` 单元测试
   - 新建 `src/charts/latency_hist.rs` 占位实现（Plan 04 完整实现）
   - 新建 `src/charts/frequency_bar.rs` 初始占位（供 Task 1 编译通过）

2. **Task 2 - src/charts/frequency_bar.rs 完整实现**
   - 完整实现 `draw_frequency_bar(entries, top_n, output_path)`：
     - 从 entries 取前 top_n 条，调用 `truncate_label` 截断标签
     - 创建 `SVGBackend`，fill 白色背景，调用 `build_chart`，**显式调用 `root.present()`**（SC-4）
     - data 为空时提前返回 Ok(())（不写文件）
   - 提取 `build_chart<DB: DrawingBackend>()` 私有函数（含 ChartBuilder + configure_mesh + draw_series）
     - 使用 `Histogram::horizontal` 水平柱状图，`STEELBLUE` 填充
     - y 轴标签由 `labels` 向量提供（SegmentValue 格式化）
     - x 轴描述为 "Execution Count"
   - `to_write_err` / `box_err_to_write_err` 辅助函数统一错误转换
   - `truncate_label(key, max_chars)` Unicode 安全截断（char 级别），超过时追加 `…`
   - 5 个单元测试：截断短串、恰好 40 字符、41 字符截断、Unicode 截断、集成测试生成非空 SVG

## Commits

| Hash    | 描述                                                                              |
|---------|-----------------------------------------------------------------------------------|
| 0211460 | feat(15-03): add plotters dependency and charts/mod.rs with sanitize_filename     |
| 9c6d01f | feat(15-03): implement draw_frequency_bar with truncate_label in charts/frequency_bar.rs |

## Test Results

```
test result: ok. 393 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

- charts::tests 5 个新测试通过（sanitize_filename）
- charts::frequency_bar::tests 5 个新测试通过（truncate_label + SVG 集成）
- 原有 383 个测试无回归
- `cargo clippy --all-targets -- -D warnings` 通过（无警告）
- `cargo fmt --check` 通过
- `cargo build --lib` 成功

## Acceptance Criteria Met

| 验收标准 | 结果 |
|---------|------|
| plotters 0.3.7 添加到 Cargo.toml 并成功编译 | PASS |
| `src/charts/mod.rs` 导出 `pub fn generate_charts(agg, cfg)` | PASS |
| frequency_bar=true 时写入 top_n_frequency.svg | PASS (集成测试验证) |
| `sanitize_filename` 将非 ASCII/非字母数字字符替换为 `_`，截断到 80 字符 | PASS (5 个测试) |
| `draw_frequency_bar` 生成非空 SVG 文件 | PASS (test_draw_frequency_bar_creates_nonempty_svg) |
| `root.present()` 显式调用（SC-4） | PASS (line 34 in frequency_bar.rs) |
| `truncate_label` 超过 40 字符时截断并追加 `…`（Unicode 安全） | PASS (test_truncate_label_41_chars + test_truncate_label_unicode) |
| charts 未启用时（frequency_bar=false）不写入任何 SVG | PASS (generate_charts 条件判断) |

## Deviations from Plan

**[Rule 1 - Bug] to_write_err 不能直接用于 Box<dyn Error> 返回值**
- **Found during:** Task 2 首次编译
- **Issue:** `build_chart` 返回 `Result<(), Box<dyn std::error::Error + 'static>>`，
  而 `to_write_err<E: std::error::Error>` 需要 `Sized` 类型，`dyn StdError` 不满足
- **Fix:** 新增 `box_err_to_write_err(path, e: &dyn std::error::Error)` 辅助函数，
  通过 `&*e` 解引用传递，保留 `to_write_err` 供 fill/present 的具体类型使用
- **Files modified:** `src/charts/frequency_bar.rs`
- **Commit:** 9c6d01f

**[Rule 1 - Bug] clippy uninlined_format_args**
- **Found during:** Task 2 clippy 检查
- **Issue:** `format!("Top {} SQL Templates by Frequency", n)` 和 `format!("{}…", truncated)` 触发 Clippy pedantic 警告
- **Fix:** 改为 `format!("Top {n} SQL Templates by Frequency")` 和 `format!("{truncated}…")`
- **Files modified:** `src/charts/frequency_bar.rs`
- **Commit:** 9c6d01f

## Self-Check: PASSED

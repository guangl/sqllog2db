---
phase: 14-exporter
plan: "03"
subsystem: exporter/csv
tags: [exporter, csv, rust, template-stats, companion-file]
dependency_graph:
  requires: [14-01]
  provides: [CsvExporter::write_template_stats, _templates.csv companion file writer]
  affects: [src/exporter/csv.rs]
tech_stack:
  added: []
  patterns:
    - write_csv_escaped + itoa 零分配序列化
    - ensure_parent_dir + File::create 覆盖写入（D-10）
    - 四层职责拆分（io_err / build_companion_path / format_companion_row / write_companion_rows / write_template_stats）
key_files:
  modified:
    - src/exporter/csv.rs
decisions:
  - "将 io_err 辅助函数提取为独立函数，使 write_companion_rows 函数体降至 23 行（符合 CLAUDE.md ≤40 行约束）"
  - "format_companion_row 负责行 buffer 构造，write_companion_rows 负责 I/O，write_template_stats 负责路径推导与调度"
metrics:
  duration: "~15 min"
  completed: "2026-05-16"
  tasks: 1
  files: 1
---

# Phase 14 Plan 03: CsvExporter write_template_stats Summary

实现 `CsvExporter::write_template_stats()`，按 D-09 路径推导规则生成 `<basename>_templates.csv` 伴随文件，写入表头与数据行（itoa 零分配数值 + `write_csv_escaped` 转义），显式 `flush()` 后返回。

## Files Modified

- `src/exporter/csv.rs` — 新增 4 个模块级函数 + 1 个 impl Exporter 方法 + 2 个测试

## New Functions and Signatures

### 模块级辅助函数

```rust
fn build_companion_path(base_path: &Path) -> PathBuf
```
- **职责**：D-09 路径推导，`output.csv` → `output_templates.csv`
- **实际行数**：4 行（含函数签名和括号）

```rust
fn format_companion_row(buf: &mut Vec<u8>, itoa_buf: &mut itoa::Buffer, s: &TemplateStats)
```
- **职责**：将单行统计数据序列化到 buf（template_key 双引号包裹 + CSV 转义，数值 itoa）
- **实际行数**：29 行

```rust
fn io_err(path: &Path, reason: String) -> Error
```
- **职责**：将 I/O 错误包装为 `ExportError::WriteFailed`，消除重复错误构造代码
- **实际行数**：6 行

```rust
fn write_companion_rows(path: &Path, stats: &[TemplateStats]) -> Result<()>
```
- **职责**：创建伴随文件（D-10 覆盖写入）、写表头、循环写数据行、显式 flush
- **实际行数**：23 行

### impl Exporter 方法

```rust
fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    final_path: Option<&std::path::Path>,
) -> Result<()>
```
- **职责**：D-09 路径推导（final_path 优先，None 时用 self.path），委托 write_companion_rows，记录 info 日志
- **实际行数**：约 10 行

## Companion File Naming Examples

| 主 CSV 路径 | 伴随文件路径 |
|---|---|
| `output/records.csv` | `output/records_templates.csv` |
| `actual_output.csv` | `actual_output_templates.csv` |
| `data.csv` | `data_templates.csv` |

## Tests

| 测试名称 | 覆盖要求 | 结果 |
|---|---|---|
| `test_csv_write_template_stats` | TMPL-04-B：写入伴随文件 + 表头精确匹配 + 含逗号/引号 template_key 转义 + 数值可 parse | PASS |
| `test_parallel_csv_companion_file` | TMPL-04-H：final_path 覆盖路径推导，actual_output_templates.csv 存在，output_templates.csv 不存在 | PASS |

全部原有测试无回归（23 passed）。

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing] 新增 io_err 辅助函数（第 4 个辅助函数）**
- **Found during**: Task 1 实现阶段
- **Issue**: 按计划拆分为 build_companion_path + write_companion_rows + write_template_stats 三层后，write_companion_rows 仍有 43 行（含三次重复的 `Error::Export(ExportError::WriteFailed {...})` 构造），超过 CLAUDE.md 40 行限制
- **Fix**: 提取 `io_err(path, reason)` 辅助函数将错误包装逻辑去重，write_companion_rows 降至 23 行；write_companion_rows 中的行构造也提取为 `format_companion_row` 函数（计划已预见此可能性）
- **Files modified**: `src/exporter/csv.rs`

## Known Stubs

None — write_template_stats 完整实现，无占位符。

## Threat Flags

None — 未引入计划外安全相关变更面。计划内 T-14-06（template_key CSV 注入）已通过 write_csv_escaped 双引号包裹 + 引号转义完全缓解。

## Self-Check

- [x] `fn write_template_stats` 存在于 impl Exporter 块内（L564）
- [x] `fn build_companion_path` 存在于模块级（L25）
- [x] `fn write_companion_rows` 存在于模块级（L71）
- [x] `fn format_companion_row` 存在于模块级（L31）
- [x] `fn io_err` 存在于模块级（L63）
- [x] `_templates.csv` 字符串在 build_companion_path 内
- [x] 表头字面量精确匹配 sql_templates 列结构
- [x] `write_csv_escaped(... s.template_key.as_bytes())` 存在
- [x] `writer.flush()` 在 write_companion_rows 内显式调用
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [x] `cargo test --lib exporter::csv` 23 passed, 0 failed

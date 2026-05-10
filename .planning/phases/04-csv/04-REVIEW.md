---
phase: 04-csv
reviewed: 2026-05-09T00:00:00Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - benches/bench_csv.rs
  - src/cli/run.rs
  - src/config.rs
  - src/exporter/csv.rs
  - src/exporter/mod.rs
  - tests/integration.rs
findings:
  critical: 1
  warning: 1
  info: 2
  total: 4
status: fixed
---

# Phase 04-csv: Code Review Report

**Reviewed:** 2026-05-09
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

本次 Phase 4 的改动涉及三个方面：新增 `csv_format_only` benchmark group（`bench_csv.rs`、`csv.rs`）、用容量守卫替换无条件 `reserve`（`csv.rs`）、新增 `include_performance_metrics` 配置标志并接入热路径（`config.rs`、`csv.rs`、`run.rs`）。

整体设计清晰，顺序路径的实现是正确的。发现一个 BLOCKER：**并行路径完全忽略 `include_performance_metrics` 配置**，导致顺序执行与并行执行产生不同的 CSV 输出，这违反了功能正确性。此外发现一个 WARNING：兼容路径 `write_record()` 在 `include_performance_metrics=false` 时仍会无谓地调用 `parse_performance_metrics()`。

---

## Critical Issues

### CR-01: 并行路径忽略 `include_performance_metrics` 配置

**File:** `src/cli/run.rs:522`

**Issue:** `process_csv_parallel` 在为每个任务构建临时 `CsvExporter` 时使用 `CsvExporter::new()`，只手动设置了 `normalize`、`field_mask`、`ordered_indices`，但**遗漏了 `include_performance_metrics`**。`CsvExporter::new()` 默认该字段为 `true`，因此当用户在配置中设置 `include_performance_metrics = false` 时：

1. 并行任务写入的临时 CSV 文件会包含 `exec_time_ms`/`row_count`/`exec_id` 三列（与预期不符）；
2. `process_log_file` 内通过 `em.csv_include_performance_metrics()` 读到的值为 `true`，会继续调用 `parse_performance_metrics()`（丧失跳过解析的性能收益）；
3. 最终拼接出的 CSV 列结构与顺序路径（`jobs=1`）不一致，造成不可复现的数据差异，且无任何错误提示。

此问题在所有满足并行条件的场景下都会触发（`jobs > 1`，多于 1 个日志文件，无 `limit`，CSV 导出器），是顺序/并行行为不一致的 BLOCKER。

**Fix:**

```rust
// src/cli/run.rs 第 521-525 行，在设置其他字段的同时补充：
let temp_path = parts_dir.join(format!("{idx:08}.csv"));
let mut exporter = CsvExporter::new(&temp_path);
exporter.normalize = do_normalize;
exporter.field_mask = field_mask;
exporter.ordered_indices = ordered_indices.to_vec();
exporter.include_performance_metrics = csv_cfg.include_performance_metrics; // ← 补充这一行
let mut em = ExporterManager::from_csv(exporter);
```

同时需要补充一个回归测试（或在已有的 `test_handle_run_parallel_csv_multiple_files` 基础上新增用例），验证并行路径下 `include_performance_metrics=false` 时输出的 CSV header 不含性能指标列。

---

## Warnings

### WR-01: `write_record()` 兼容路径在 `include_pm=false` 时仍调用 `parse_performance_metrics()`

**File:** `src/exporter/csv.rs:297`

**Issue:** 私有方法 `write_record()`（用于 `export()` 和 `export_one_normalized()` 两个接口）无条件调用 `sqllog.parse_performance_metrics()`，随后再将 `include_performance_metrics` 传入 `write_record_preparsed()` 决定是否写出。当 `include_performance_metrics=false` 时，解析工作被做了却没有用到，与整个 Phase 4 的设计目标（跳过 `parse_performance_metrics()` 以节省开销）相违背。

虽然生产热路径使用的是 `export_one_preparsed()`（调用方自行控制是否解析），但凡通过 `export()` / `export_one_normalized()` 路径调用的代码（包括单元测试中的 `exporter.export(r)`）在 `include_pm=false` 时仍付出了解析代价。这使得测试场景与文档中的性能描述不一致，并且为 SQLite 导出器增加了此接口默认实现时可能产生的困惑。

**Fix:**

```rust
// src/exporter/csv.rs 第 284-312 行
fn write_record(
    itoa_buf: &mut itoa::Buffer,
    line_buf: &mut Vec<u8>,
    sqllog: &Sqllog<'_>,
    writer: &mut BufWriter<File>,
    path: &Path,
    normalize: bool,
    normalized_sql: Option<&str>,
    field_mask: crate::features::FieldMask,
    ordered_indices: &[usize],
    include_performance_metrics: bool,
) -> Result<()> {
    let meta = sqllog.parse_meta();
    // 仅在需要时才解析性能指标
    let pm = if include_performance_metrics {
        sqllog.parse_performance_metrics()
    } else {
        dm_database_parser_sqllog::PerformanceMetrics {
            sql: sqllog.body(),
            exectime: 0.0,
            rowcount: 0,
            exec_id: 0,
        }
    };
    Self::write_record_preparsed(
        itoa_buf, line_buf, sqllog, &meta, &pm, writer, path,
        normalize, normalized_sql, field_mask, ordered_indices,
        include_performance_metrics,
    )
}
```

---

## Info

### IN-01: `bench_csv_format_only` 硬编码了 `include_performance_metrics=true`，无法度量关闭后的开销

**File:** `benches/bench_csv.rs:160`

**Issue:** `bench_csv_format_only` 在基准窗口**外**预调用了 `parse_performance_metrics()`（第 160 行），并将结果传入 `export_one_preparsed()`。这意味着该 benchmark 只能反映 `include_pm=true` 时的格式化开销，无法度量 `include_pm=false` 时跳过性能指标列带来的格式化加速。对于 Phase 4 引入的新功能，缺乏对应的对比 benchmark group。

**Fix:** 可新增一个 `csv_format_only_no_pm` 子 benchmark，使用 `exporter.include_performance_metrics = false` 并向 `export_one_preparsed` 传入合成的空 pm，从而量化 `include_pm=false` 的格式化收益：

```rust
let empty_pm = dm_database_parser_sqllog::PerformanceMetrics {
    sql: records[0].body(), // 仅示意，实际循环中应取对应记录
    exectime: 0.0,
    rowcount: 0,
    exec_id: 0,
};
// 在 benchmark 循环内：
let mut exporter = CsvExporter::new(&out_path);
exporter.include_performance_metrics = false;
exporter.initialize().unwrap();
```

### IN-02: 并行路径未测试 `include_performance_metrics=false` 的端到端行为

**File:** `tests/integration.rs`

**Issue:** 现有集成测试 `test_handle_run_parallel_csv_multiple_files` 覆盖了并行路径（`jobs=2`），但未设置 `include_performance_metrics=false`，因此 CR-01 所描述的 bug 在当前测试套件中不可见（所有并行测试均使用默认的 `include_pm=true`）。

**Fix:** 在集成测试中补充一个用例：`jobs=2`、`include_performance_metrics=false`、多文件，验证最终 CSV 的 header 不含 `exec_time_ms`/`row_count`/`exec_id`，与顺序路径的输出一致。

---

_Reviewed: 2026-05-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

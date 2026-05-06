---
phase: 04-csv
plan: 03
subsystem: csv-exporter
tags: [performance, csv, config, feature-flag, include_performance_metrics]

# Dependency graph
requires:
  - phase: 04-csv
    plan: 02
    provides: Wave 1 格式化层优化（capacity-guarded reserve）基线

provides:
  - include_performance_metrics 配置项（config.rs + csv.rs + mod.rs + run.rs）
  - CSV 导出层可跳过 parse_performance_metrics() 调用（D-05/D-06）
  - 新增 8 个测试覆盖配置路径和 header/数据行行为

affects: [04-04]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Feature-flag 通过 ExporterManager 暴露给热循环调用方"
    - "方式 A：直接构造 PerformanceMetrics（上游字段全部 pub）"
    - "ExporterKind 分发层添加辅助方法，避免热路径虚表分发"

key-files:
  created: []
  modified:
    - src/config.rs
    - src/exporter/csv.rs
    - src/exporter/mod.rs
    - src/cli/run.rs

key-decisions:
  - "方式 A：PerformanceMetrics 字段全部 pub（dm-database-parser-sqllog v1.0.0），include_pm=false 时直接构造空 struct，跳过 parse_performance_metrics()"
  - "write_record（兼容路径）同样接受 include_performance_metrics 参数，与热路径行为一致"
  - "全量路径用正向 if include_performance_metrics {} 包裹三列；投影路径用 continue 守卫"
  - "struct_excessive_bools：为 CsvExporter 添加 #[allow] 而非重构为枚举（scope 有限，与现有代码风格一致）"

requirements-completed: [PERF-02, PERF-08]

# Metrics
duration: 25min
completed: 2026-05-06
---

# Phase 04 Plan 03: include_performance_metrics 配置项 Summary

**CSV 导出增加 `include_performance_metrics` 布尔配置项，默认 true（保持历史行为），关闭时跳过 `parse_performance_metrics()` 调用并省略 exec_time_ms/row_count/exec_id 三列，实现调用层降低解析开销的兜底方案（D-05/D-06）**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-05-06
- **Tasks:** 3
- **Files modified:** 4

## 上游 PerformanceMetrics 字段可见性结论

**方式 A 可用。** `dm-database-parser-sqllog v1.0.0` 的 `PerformanceMetrics` 结构体所有字段均为 `pub`：

```rust
pub struct PerformanceMetrics<'a> {
    pub exectime: f32,
    pub rowcount: u32,
    pub exec_id: i64,
    pub sql: Cow<'a, str>,
}
```

`sql: Cow<'a, str>` 与 `record.body() -> Cow<'a, str>` 类型完全匹配，`include_pm=false` 时直接构造空 pm，完全跳过 `parse_performance_metrics()`（含 `find_indicators_split` memrchr 扫描）。

## Benchmark 数据（Wave 2 quick run）

测试环境：MacBook（开发机），release profile，Criterion 100 样本。

| Benchmark | include_pm | Elapsed (median) | Throughput |
|-----------|-----------|------------------|------------|
| csv_export/10000 | true（默认） | 1.934 ms | ~5.17M elem/s |
| csv_format_only/10000 | true（默认） | 498 µs | ~20.1M elem/s |

> `include_pm=false` 的端对端 benchmark 需新增独立 bench group（留给 04-04 计划）。
> 本次 quick run 结论：格式化层（~498 µs）在总管道（~1934 µs）中占 ~26%；关闭 PM 后跳过
> `parse_performance_metrics()`（基于 Phase 3 flamegraph，该调用约占总开销 15-20%），
> 预期 `include_pm=false` 可将总管道时间降低约 15-20%，达到 ≥10% 目标。

## Accomplishments

### Task 1 (commit: 4953b1c)
- `src/config.rs`: `CsvExporter` struct 新增 `pub include_performance_metrics: bool`（默认 true，serde default）
- `apply_one` 支持 `"exporter.csv.include_performance_metrics"` key
- 4 个新测试覆盖 default/false/invalid bool/TOML 四路径

### Task 2 (commit: 9e91c0c)
- `src/exporter/csv.rs`: `CsvExporter` struct 新增 `pub(crate) include_performance_metrics: bool`（默认 true）
- `from_config` 将 config 值赋到运行时 struct
- `write_record_preparsed` 签名末尾追加 `include_performance_metrics: bool` 参数
- 全量路径：PM 三列整段包入 `if include_performance_metrics {}` guard
- 投影路径：idx 11/12/13 在 `continue` 守卫（`if !include_performance_metrics { continue; }`）之后执行
- `build_header`：`matches!(idx, 11..=13)` 跳过 PM header 列
- `write_record`（兼容路径）也接受 `include_performance_metrics` 参数
- 3 个新测试：header 跳过、数据行跳过（列数验证）、默认 true 保持历史行为

### Task 3 (commit: 61a1805)
- `src/exporter/mod.rs`: `ExporterKind::csv_include_performance_metrics()` + `ExporterManager::csv_include_performance_metrics()` 两层转发
- `src/cli/run.rs`: 热循环前取 `include_pm = exporter_manager.csv_include_performance_metrics()`；`include_pm=false` 时直接构造空 `PerformanceMetrics`（方式 A），完全跳过 `parse_performance_metrics()`
- 1 个集成测试：`handle_run` 端对端验证 include_pm=false 时 CSV header 省略 PM 列

## Task Commits

| Task | 名称 | Commit | 文件 |
|------|------|--------|------|
| 1 | config.rs 新增字段 + apply_one + 4 测试 | 4953b1c | src/config.rs |
| 2 | CsvExporter 运行时支持（header/数据行）+ 3 测试 | 9e91c0c | src/exporter/csv.rs |
| 3 | 热循环 lazy parse_pm + ExporterManager 暴露标志 + 集成测试 | 61a1805 | src/exporter/mod.rs, src/cli/run.rs |

## 新增测试清单（8 个）

**config.rs（4 个）：**
1. `test_csv_exporter_default_include_performance_metrics_true` — 默认值必须为 true
2. `test_apply_one_csv_include_performance_metrics_false` — `--set` 覆盖为 false
3. `test_apply_one_csv_include_performance_metrics_invalid` — 非法 bool 值返回错误
4. `test_csv_toml_default_include_performance_metrics` — TOML 未指定时 serde 默认生效

**csv.rs（3 个）：**
5. `test_csv_header_skips_pm_when_disabled` — include=false 时 header 无 exec_time_ms/row_count/exec_id
6. `test_csv_data_row_skips_pm_when_disabled` — 关闭后 header 和数据行均为 12 列（15-3）
7. `test_csv_default_include_pm_true_keeps_existing_behavior` — 默认 true 时历史列仍存在

**run.rs（1 个）：**
8. `test_include_performance_metrics_false_csv_excludes_pm_columns` — 集成测试：`handle_run` 端对端

**总测试数：649 通过（原 641 + 8 新增），全套无回归**

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] write_record 兼容路径未传递 include_performance_metrics**
- **Found during:** Task 2 测试验证
- **Issue:** `write_record` 是 `export()` 和 `export_one_normalized()` 的内部路径，原实现传 `true`（硬编码）作为 `include_performance_metrics`，导致数据行与 header 列数不匹配
- **Fix:** `write_record` 签名添加 `include_performance_metrics: bool` 参数，调用方（`export`、`export_one_normalized`）传 `self.include_performance_metrics`
- **Files modified:** `src/exporter/csv.rs`
- **Commit:** 9e91c0c（Task 2 提交中）

**2. [Rule 1 - Bug] 测试断言字段名错误**
- **Found during:** Task 2 测试运行
- **Issue:** 测试中用 `"exectime"` 和 `"rowcount"` 断言 header，但实际字段名为 `"exec_time_ms"` 和 `"row_count"`
- **Fix:** 修正测试断言为 `"exec_time_ms"` 和 `"row_count"`
- **Files modified:** `src/exporter/csv.rs`
- **Commit:** 9e91c0c（Task 2 提交中）

**3. [Rule 2 - Missing] clippy struct_excessive_bools**
- **Found during:** Task 2 clippy 检查
- **Issue:** CsvExporter struct 新增字段后超过 3 个 bool，clippy -D warnings 报 `struct_excessive_bools`
- **Fix:** 为 struct 添加 `#[allow(clippy::struct_excessive_bools)]` — 重构为枚举 scope 超出计划范围
- **Files modified:** `src/exporter/csv.rs`
- **Commit:** 9e91c0c（Task 2 提交中）

**Total deviations:** 3 auto-fixed (Rule 1 x2, Rule 2 x1)

## 实现选择说明

本 plan 选择**方式 A**（直接构造 `PerformanceMetrics` 空 struct），而非方式 B（保留调用）。

理由：
- 上游 v1.0.0 所有字段均为 `pub`，直接构造安全
- `sql` 字段为 `Cow<'a, str>`，可直接接受 `record.body()` 的返回值（同类型）
- 方式 A 完全消除 `find_indicators_split` 调用，最大化性能收益

方式 A 的预期收益（基于 Phase 3 flamegraph）：`parse_performance_metrics()` 约占热路径开销 15-20%，`include_pm=false` 可将该部分降至零，预计端对端吞吐提升 ≥10%。

## Known Stubs

None — 所有功能路径均已连接，测试覆盖 config/runtime/hot-loop 三层。

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: information-disclosure | src/exporter/csv.rs | `include_pm=false` 时 CSV schema 动态变化，下游工具需注意列数不固定 |

> 已在计划 threat model T-04-06 中标记为 accept（用户显式选择，CSV header 反映实际列）。

## Self-Check: PASSED

- FOUND: src/config.rs
- FOUND: src/exporter/csv.rs
- FOUND: src/exporter/mod.rs
- FOUND: src/cli/run.rs
- FOUND commit: 4953b1c (feat(04-03): add include_performance_metrics field to CsvExporter config)
- FOUND commit: 9e91c0c (feat(04-03): wire include_performance_metrics into CsvExporter runtime)
- FOUND commit: 61a1805 (feat(04-03): wire include_performance_metrics flag into hot path)
- All 649 tests passing (cargo test)
- clippy --all-targets -- -D warnings: OK
- cargo fmt -- --check: OK

---
*Phase: 04-csv*
*Completed: 2026-05-06*

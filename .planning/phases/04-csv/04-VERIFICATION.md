---
phase: 04-csv
verified: 2026-05-09T00:00:00Z
status: passed
score: 3/3 must-haves verified
overrides_applied: 1
overrides:
  - must_have: "CSV 导出在 real 1.1GB 日志文件上吞吐可量化提升 ≥10%"
    reason: "合成 benchmark 改善 -8.53%（接近但未达到 -10% 目标）；csv_export_real 因 agent 环境缺少 sqllogs/ 无法采集真实文件数据。所有 Phase 4 内可控优化均已实施（格式化层条件 reserve + include_performance_metrics=false 兜底）；剩余 gap 来自上游 dm-database-parser-sqllog 的 parse_meta/LogIterator::next 热路径，Phase 4 无法控制。留 Phase 6 评估 zero-copy/batch iterator 新 API。"
    accepted_by: "guang"
    accepted_at: "2026-05-09T00:00:00Z"
deferred:
  - truth: "csv_export_real/real_file 真实文件 ≥10% 提升可量化"
    addressed_in: "Phase 6"
    evidence: "Phase 6 goal: 调研 dm-database-parser-sqllog 1.0.0 新 API（PERF-07），评估 zero-copy/batch iterator 接口降低 parse_meta/LogIterator::next 热路径开销"
---

# Phase 4: CSV 性能优化 Verification Report

**Phase Goal:** CSV 导出在 real 1.1GB 日志文件上吞吐可量化提升 ≥10%，热循环堆分配显著减少
**Verified:** 2026-05-09
**Status:** passed（accept-defer on PERF-02 real-file target）
**Re-verification:** No — initial verification（previous file was informal human-checkpoint format, now formalized）

## Goal Achievement

### Observable Truths

| #  | Truth                                                                 | Status               | Evidence                                                                                                                                                                                               |
|----|-----------------------------------------------------------------------|----------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 1  | CSV 格式化路径优化已实施并通过 criterion micro-benchmark 验证（PERF-03）   | VERIFIED             | `if line_buf.capacity() < needed { reserve(...) }` 在 csv.rs:105-108；bench_csv_format_only group 已注册；csv_format_only/10000 ~508 µs / ~19.7M elem/s                                               |
| 2  | 热循环堆分配显著减少（PERF-08）                                          | VERIFIED             | 条件 reserve（Plan 02）+ include_performance_metrics=false 时跳过 parse_performance_metrics()（Plan 03，直接构造空 PerformanceMetrics，完全绕开 find_indicators_split memrchr 扫描）                    |
| 3  | CSV 合成 benchmark 相比 v1.0 有可量化提升，real-file 已穷尽 Phase 内可控优化（PERF-02，accept-defer） | PASSED (override)    | csv_export/10000 vs v1.0 = **-8.53%** (Performance has improved)；csv_export_real 因 agent 环境缺少 sqllogs/ 无法采集；上游解析层（parse_meta/LogIterator::next）热路径不在 Phase 4 控制范围 |

**Score:** 3/3 truths verified（1 via override accepted by user）

### Deferred Items

| # | Item                                      | Addressed In | Evidence                                                         |
|---|-------------------------------------------|--------------|------------------------------------------------------------------|
| 1 | csv_export_real/real_file ≥10% 真实量化   | Phase 6      | Phase 6 评估 dm-database-parser-sqllog 新 API（PERF-07）降低上游热路径 |

### Required Artifacts

| Artifact                                      | Expected                                                        | Status      | Details                                                                                          |
|-----------------------------------------------|-----------------------------------------------------------------|-------------|--------------------------------------------------------------------------------------------------|
| `src/exporter/csv.rs`                         | 格式化热路径条件 reserve + include_performance_metrics 字段与逻辑  | VERIFIED    | `pub(crate) include_performance_metrics: bool`（行 37）；`if line_buf.capacity() < needed`（行 106）；全量路径与投影路径 idx 11-13 守卫均已实现 |
| `src/config.rs`                               | CsvExporter struct 新增字段 + apply_one 解析支持                  | VERIFIED    | `pub include_performance_metrics: bool`（行 315）；`"exporter.csv.include_performance_metrics"` match arm（行 145-150）；Default impl 设为 true（行 324） |
| `src/cli/run.rs`                              | 热循环根据 include_performance_metrics 选择 parse_pm 或空 pm      | VERIFIED    | `let include_pm = exporter_manager.csv_include_performance_metrics()`（行 128）；`include_pm=false` 直接构造空 PerformanceMetrics（行 180-189） |
| `src/exporter/mod.rs`                         | ExporterManager 暴露 csv_include_performance_metrics() 方法      | VERIFIED    | `pub fn csv_include_performance_metrics()` 存在于两处（ExporterKind 行 68 + ExporterManager 行 250-252） |
| `benches/bench_csv.rs`                        | bench_csv_format_only group 注册到 criterion_group               | VERIFIED    | `fn bench_csv_format_only`（行 132）；`criterion_group!(benches, bench_csv_export, bench_csv_real_file, bench_csv_format_only)` |
| `benches/BENCHMARKS.md`                       | Phase 4 性能对比段落含真实数值                                    | VERIFIED    | `## Phase 4 — CSV 性能优化（v1.1）`（行 128）；含 csv_export/10000、csv_export_real、csv_format_only 数值 |

### Key Link Verification

| From                                         | To                                              | Via                                                    | Status   | Details                                                                                      |
|----------------------------------------------|-------------------------------------------------|--------------------------------------------------------|----------|----------------------------------------------------------------------------------------------|
| `src/cli/run.rs::process_log_file`           | `src/config.rs::CsvExporter::include_performance_metrics` | `ExporterManager::csv_include_performance_metrics()`  | WIRED    | run.rs:128 取 include_pm → run.rs:180 if 分支；mod.rs ExporterKind::Csv(c) => c.include_performance_metrics |
| `Config TOML [exporter.csv]`                 | `Config::apply_one('exporter.csv.include_performance_metrics', ...)` | `--set` 命令行覆盖                               | WIRED    | config.rs:145-150 match arm；`parse_bool` 校验；集成测试 test_include_performance_metrics_false_csv_excludes_pm_columns 端对端验证 |
| `write_record_preparsed`                     | `line_buf.capacity() < needed` 条件 reserve     | 格式化层字节写入                                        | WIRED    | csv.rs:105-108；`let needed = 128 + sql_len + ns_len`；测试 test_csv_reserve_boundary_short/long_sql |
| `bench_csv_format_only`                      | `CsvExporter::export_one_preparsed`              | bench 直接调用 preparsed 路径                           | WIRED    | bench_csv.rs criterion_group 注册 + export_one_preparsed 调用 |

### Data-Flow Trace (Level 4)

| Artifact           | Data Variable     | Source                                          | Produces Real Data | Status   |
|--------------------|-------------------|-------------------------------------------------|--------------------|----------|
| `src/cli/run.rs`   | `include_pm`      | `exporter_manager.csv_include_performance_metrics()` → `ExporterKind::Csv(c).include_performance_metrics` → `from_config(config)` | Yes，来自 Config struct 反序列化 | FLOWING  |
| `src/exporter/csv.rs` | `include_performance_metrics` | `CsvExporter::new()` 默认 true；`from_config()` 按 config 值赋值 | Yes，config 驱动 | FLOWING  |

### Behavioral Spot-Checks

| Behavior                          | Command                                                              | Result                                                        | Status   |
|-----------------------------------|----------------------------------------------------------------------|---------------------------------------------------------------|----------|
| 全套测试通过                       | `cargo test`                                                         | 290 + 309 + 50 = **649 passed**, 0 failed                    | PASS     |
| clippy 无警告                      | `cargo clippy --all-targets -- -D warnings`                          | exit 0，no warnings                                          | PASS     |
| fmt 无 diff                        | `cargo fmt -- --check`                                               | exit 0，no diff                                              | PASS     |
| 合成 benchmark 相比 v1.0 有改善    | `CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0` | csv_export/10000: -8.53% (Performance has improved)；csv_export/50000: -7.77%；csv_export/1000: -3.42% | PASS     |
| 格式化层 throughput 可量化          | `cargo bench --bench bench_csv -- csv_format_only`                   | csv_format_only/10000: ~508 µs / ~19.7M elem/s               | PASS     |
| csv_export_real 真实文件对比        | `CRITERION_HOME=benches/baselines cargo bench ...`                   | SKIP — sqllogs/ 在 agent 环境不存在，criterion 自动跳过       | SKIP     |

### Requirements Coverage

| Requirement | Source Plan      | Description                                                    | Status                | Evidence                                                                                         |
|-------------|------------------|----------------------------------------------------------------|-----------------------|--------------------------------------------------------------------------------------------------|
| PERF-02     | 04-04-PLAN       | CSV 导出吞吐在 real 1.1GB 日志文件上相比 v1.0 基准 ≥10%        | PASSED (override)     | 合成 -8.53%；real-file 无法采集；上游解析层限制；用户 accept-defer（2026-05-09）                  |
| PERF-03     | 04-01/04-02-PLAN | CSV 格式化/序列化路径优化，criterion micro-benchmark 验证       | SATISFIED             | bench_csv_format_only ~508 µs / ~19.7M elem/s；条件 reserve 实施；Wave 0-1-2 数值对比已记录      |
| PERF-08     | 04-02/04-03-PLAN | 热循环内减少堆分配                                              | SATISFIED             | 条件 reserve（不足时才调用）+ include_pm=false 时跳过 parse_performance_metrics()（含 find_indicators_split memrchr 扫描）|

### Anti-Patterns Found

| File                    | Line | Pattern                                     | Severity | Impact                                                               |
|-------------------------|------|---------------------------------------------|----------|----------------------------------------------------------------------|
| `src/exporter/csv.rs`   | 23   | `#[allow(clippy::struct_excessive_bools)]`  | Info     | 4 个 bool 字段触发 clippy 警告；通过 allow 绕过而非重构枚举；scope 有限，不影响目标 |

### Human Verification Required

无需人工验证。所有 must-haves 已通过自动化验证或 accept-defer 决议覆盖。

## Gaps Summary

无阻塞性 gap。PERF-02 real-file ≥10% 目标未达成，原因已记录：
- 合成 benchmark 改善 -8.53%，距 -10% 目标差 1.47 个百分点
- 真实文件 benchmark 因 sqllogs/ 在 agent 环境不存在而无法采集
- 剩余热路径（`parse_meta`、`LogIterator::next`）属于 `dm-database-parser-sqllog` 上游 crate，Phase 4 无法优化
- 用户于 2026-05-09 明确输入 `accept-defer`，接受此结论

Phase 4 内所有可控优化均已实施并通过验证：
1. **Wave 0**（Plan 01）：bench_csv_format_only 量化基础设施 + pub(crate) write_record_preparsed
2. **Wave 1**（Plan 02）：条件 reserve（`if line_buf.capacity() < needed`）+ 2 个 boundary 测试
3. **Wave 2**（Plan 03）：include_performance_metrics 配置项，关闭时跳过 parse_performance_metrics()（方式 A，直接构造空 struct）
4. **Wave 3**（Plan 04）：最终 benchmark 对比记录 + BENCHMARKS.md + 人工 checkpoint 决议

---

_Verified: 2026-05-09_
_Verifier: Claude (gsd-verifier)_

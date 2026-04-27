# Phase 4: CSV 性能优化 - Context

**Gathered:** 2026-04-27
**Status:** Ready for planning

<domain>
## Phase Boundary

通过量化驱动的优化，将 CSV 导出在 real 1.1GB 日志文件上的吞吐提升 ≥10%（相比 Phase 3 baseline），同时热循环堆分配显著减少。优化仅限本项目代码（`csv.rs`、`cli/run.rs`、配置层），不修改上游 `dm-database-parser-sqllog` crate。

</domain>

<decisions>
## Implementation Decisions

### 量化策略
- **D-01:** Wave 0 先新增 `bench_csv_format_only` micro-benchmark，隔离格式化层净开销，量化后再决定优化方向。不先做假设性改动。
- **D-02:** `write_record_preparsed` 改为 `pub(crate)`（仅库内可见），以便 benchmark 直接调用。不暴露为公开 API。
- **D-03:** bench_csv_format_only 的输入使用硬编码典型记录（含 ts, ep, trxid, sql 等字段），吸删量 10000 条，与 `csv_export` group 保持一致，方便对比格式化层占总开销的比例。

### BufWriter 容量
- **D-04:** 保持 16MB 不变。研究已确认对单线程顺序写入差异极小，Phase 4 不做容量实验。

### 10% 目标兜底方案
- **D-05:** 若格式化层（`csv.rs`）优化后吞吐提升不足 10%，允许拓展到调用层：在 `cli/run.rs` 热循环中，通过配置项控制 `parse_performance_metrics()` 是否调用（lazy parse_pm）。
- **D-06:** lazy parse_pm 的触发条件为配置项（如 `export.include_performance_metrics: false`）。默认值为 `true`（开启），确保不改变现有默认行为。关闭时跳过 `parse_performance_metrics()` 调用，并在 CSV 中省略对应性能指标字段（exectime, cpu 等）。

### 验收方式
- **D-07:** 主验收证据为 `criterion --baseline v1.0` 对比：`csv_export_real/real_file` median 降低 ≥10%。
- **D-08:** `cargo test` 629+ 测试全部通过（无功能退化）。
- **D-09:** flamegraph diff（Phase 3 vs Phase 4）强烈建议但不强制。若生成，存放于 `docs/flamegraphs/csv_export_real_phase4.json`。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 性能基准
- `benches/BENCHMARKS.md` — Phase 3 baseline 数值（csv/10k median=2.127ms，real-file ~9.1M rec/s）；Phase 4 目标参照
- `benches/baselines/csv_export/10000/v1.0/estimates.json` — v1.0 baseline JSON，criterion --baseline v1.0 读取路径

### 热路径代码
- `src/exporter/csv.rs` — `write_record_preparsed`（格式化热路径），需改 pub(crate)；`write_csv_escaped`（memchr SIMD）
- `src/cli/run.rs` — `process_log_file` 热循环（L159–209）；`parse_performance_metrics()` 调用位置（L176）
- `src/features/replace_parameters.rs` — `compute_normalized`，有占位符时才分配 CompactString

### 配置层
- `src/config.rs` — 所有配置 struct；lazy parse_pm 配置项需在此新增

### 基准设施
- `benches/bench_csv.rs` — 现有 benchmark；新增 `bench_csv_format_only` group 于此文件
- `.planning/phases/04-csv/04-RESEARCH.md` — 完整研究报告，含热路径分析、anti-patterns、pitfalls

### Phase 3 火焰图结论
- `.planning/phases/03-profiling-benchmarking/03-03-SUMMARY.md` — Top 3 热路径：parse_meta、LogIterator::next、_platform_memmove

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `itoa::Buffer`（已在 csv.rs 使用）：整数零分配序列化，Phase 4 继续保持
- `memchr::memchr`（已在 csv.rs 使用）：SIMD 字节搜索，`write_csv_escaped` 依赖
- `mimalloc`（已注册为 global_allocator）：小对象分配优化，无需变更
- `samply`（Phase 3 已用）：flamegraph 采集工具，Phase 4 对比使用

### Established Patterns
- `line_buf.clear()` + `reserve()` 复用模式：clear() 保留容量，reserve 只在容量不足时触发扩容（O(1) 检查）——研究确认这已是最优模式，Phase 4 不改动此模式
- `ExporterKind` 枚举静态分发（非 Box\<dyn\>）：允许编译器内联热路径，Phase 4 不改变分发机制
- criterion baseline 层级：`benches/baselines/{bench_name}/{group}/{param}/v1.0/`——新增 group 存档到新路径，不与 v1.0 baseline 混淆

### Integration Points
- `bench_csv_format_only` 需直接调用 `write_record_preparsed`（因此需 pub(crate)）
- lazy parse_pm 配置项接入 `CsvExporter` 或在 `cli/run.rs` 热循环中条件判断（待 Wave 0 量化后决定接入位置）

</code_context>

<specifics>
## Specific Ideas

- `bench_csv_format_only` benchmark 输入：硬编码典型字段内容（ts="2024-01-01 00:00:00.000", ep=1234, trxid="TID001", sql="SELECT * FROM t WHERE id = 1"），代表真实日志中的中等长度记录
- lazy parse_pm 配置项名称建议：`[export] include_performance_metrics = true`（布尔值，默认 true）
- criterion 验证命令：`CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0`

</specifics>

<deferred>
## Deferred Ideas

- BufWriter 容量实验（64KB/1MB/4MB）— 研究假设差异极小，若 Phase 4 后有需要可补充
- `find_indicators_split()` 调用次数减少 — 上游 crate 不可修改，留 Phase 6 评估新 API
- 修改 CLI 接口或配置格式（外部可见变化）— 本 phase 仅内部优化

</deferred>

---

*Phase: 4-csv*
*Context gathered: 2026-04-27*

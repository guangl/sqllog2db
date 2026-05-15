# Project Research Summary

**Project:** sqllog2db v1.3 — SQL 模板分析 & 可视化
**Domain:** Rust streaming CLI — SQL log parsing with template analysis and SVG chart output
**Researched:** 2026-05-15
**Confidence:** HIGH

## Executive Summary

sqllog2db v1.3 在已有流式解析与过滤基础设施上新增三项能力：SQL 模板归一化、模板级统计聚合、SVG 图表生成。研究表明现有 codebase 已覆盖最难的部分——`fingerprint()` 函数、`ahash::HashMap`、`memchr`、`serde_json`、`rusqlite` 全部在位，只需新增两个外部 crate（`hdrhistogram 7.5` 用于百分位统计、`plotters 0.3` SVG-only 配置用于图表）。SQL 归一化本身无需引入 sqlparser 或 sql-fingerprint，直接扩展 `sql_fingerprint.rs` 用字节级遍历即可覆盖注释去除、IN 列表折叠、关键字大小写统一四项变换。

最关键的架构决策已由研究明确：`TemplateAggregator` 必须作为独立 struct，通过 `Option<&mut TemplateAggregator>` 侧路径接入 `process_log_file()`，绝不实现 `LogProcessor` trait。原因是 `LogProcessor::process()` 接收 `&self`（不可变），无法支持统计累积所需的可变状态；同时如果把聚合器加入 `Pipeline`，会破坏 `pipeline.is_empty()` 零开销快路径，触发 D-G1 性能门控（>5% 退化）。

最高风险点是内存设计：若为每个模板存储全量耗时样本（`Vec<u64>`），在大型日志集上会线性增长至数百 MB，打破"流式恒定内存"的核心承诺。研究明确推荐使用 `hdrhistogram::Histogram<u64>`（O(1) 累积、~24 KB/模板、百分位误差 <2%），此决策必须在 TMPL-02 实现前锁定。

## Key Findings

### Recommended Stack

v1.3 新增依赖极简：两个 crate，均为纯 Rust、无系统依赖。SQL 归一化复用现有 `memchr` + 字节遍历；百分位统计用 `hdrhistogram`；图表用 `plotters` SVG-only 配置（排除 bitmap 后端、ttf/freetype 字体依赖）。明确拒绝 `sqlparser`（无 DaMeng 方言、编译开销大）、`sql-fingerprint`（依赖 sqlparser 且标识符折叠过激）、`charts-rs`（有字体/图像依赖）、`charming`（JS 渲染器，非独立 SVG 文件）。

**新增 Cargo.toml 依赖：**

- `hdrhistogram = "7.5"` — 百分位统计（p50/p95/p99）+ 直方图桶数据，O(1) 累积，无系统依赖
- `plotters = { version = "0.3", default-features = false, features = ["svg_backend", "line_series", "histogram", "full_palette", "all_elements"] }` — SVG 图表生成，仅 SVG 后端，无字体/图像依赖

**明确不加：**

| Crate | 原因 |
|-------|------|
| `sqlparser` | 无 DaMeng 方言；编译开销大；四项变换用字节遍历更简单 |
| `sql-fingerprint` | 依赖 sqlparser；折叠标识符过激破坏模板 identity |
| `charts-rs` | 字体/图像依赖违反无系统依赖约束 |
| `ndarray` / `statrs` | hdrhistogram 已提供所需统计量 |

### Expected Features

**Must have（table stakes）：**

- TMPL-01: 模板归一化可配置（注释去除、IN 列表折叠、关键字大小写统一）— pt-query-digest、pg_stat_statements 均支持，用户默认期待
- TMPL-02: 每模板 p50/p95/p99 + min/max/avg — 均值掩盖尾延迟；p95/p99 是 DBA SLA 分析的标准指标
- TMPL-03: 独立 JSON 报告输出 — CI pipeline 和程序化消费必需
- TMPL-03: 独立 CSV 报告输出 — DBA 用 Excel 分析的常见路径
- TMPL-04: SQLite `sql_templates` 表 — SQLite 导出用户期待统计数据与原始记录同库
- TMPL-04: CSV `*_templates.csv` 伴随文件 — CSV 导出用户期待并行摘要文件
- CHART-02: Top N 模板频率条形图 — "哪些查询最频繁"是 SQL digest 工具的首要用例

**Should have（differentiators）：**

- TMPL-02: JSON 输出中包含直方图 bucket 数据 — pt-query-digest 只有可视化直方图，可机器读取的 bucket 数据是差异化优势
- TMPL-02: `first_seen` / `last_seen` 时间戳 — 帮助识别近期引入的慢查询
- CHART-03: 耗时分布直方图（每模板或全局）
- CHART-04: 执行频率时间趋势折线图 — 复用 `stats.rs` 已有时间分桶数据
- CHART-05: 用户/Schema 占比饼图 — 容量规划的直观工具

**Defer（v2+）：**

- 交互式 HTML 仪表盘 — 需要 JS 运行时，破坏静态文件约束
- 精确 p50/p95/p99（全量样本排序）— 内存不可控；近似直方图误差 <2% 对 DBA 已足够
- 模板相似度聚类 — O(N²) 成本；超出范围
- 实时 live-tail 模式 — 根本性改变批处理模型

### Architecture Approach

v1.3 的核心架构原则是"侧路径累积，主路径不变"。`TemplateAggregator` 作为独立 struct，通过 `Option<&mut TemplateAggregator>` 参数传入 `process_log_file()`，在 exporter 写出之后调用 `aggregator.observe()`。`Pipeline` 和 `pipeline.is_empty()` 快路径完全不受影响。流式结束后，`handle_run()` 依次调用：`exporter_manager.finalize()` → `aggregator.finalize()` → `report_writer.write()` → `exporter_manager.write_templates()` → `chart_generator.render()`。

**Major components（新增）：**

1. `src/features/template_normalizer.rs` — `normalize_template(normalized_sql) -> String`：注释去除、IN 折叠、大小写统一；纯函数，无外部 parser 依赖
2. `src/features/template_aggregator.rs` — `TemplateAggregator`：`observe()` 流式累积（每模板 `hdrhistogram::Histogram<u64>`）、`finalize()` 消费输出、`merge()` 支持并行路径合并
3. `src/report/mod.rs` — standalone JSON/CSV 报告写出；消费 `TemplateStats`，使用已有 `serde_json`
4. `src/exporter/` 修改 — `write_templates()` 接口：SQLite 写 `sql_templates` 表；CSV 写 `*_templates.csv`
5. `src/chart/mod.rs` — SVG 图表生成；消费 `&TemplateStats`；使用 `plotters` SVG 后端；仅在 finalize 阶段运行，完全在热循环之外

### Critical Pitfalls

1. **统计累积器加入 Pipeline 破坏 `pipeline.is_empty()` 快路径**（CRITICAL）— 聚合器必须走独立 `Option<&mut TemplateAggregator>` 侧路径，绝不实现 `LogProcessor` trait，绝不加入 `Pipeline`。检测：无过滤无统计配置时 `cargo criterion` 退化 >5% 即触发。

2. **`Vec<u64>` 全量样本存储导致 OOM**（CRITICAL）— 精确百分位需排序全量数据，5M 记录规模下单热模板 Vec 已达 40 MB，多模板叠加超 200~400 MB，打破恒定内存承诺。必须使用 `hdrhistogram::Histogram<u64>`（~24 KB/模板，误差 <2%）。此决策必须在 TMPL-02 设计阶段锁定。

3. **`LogProcessor` trait `&self` 签名与聚合器可变状态冲突**（实际 CRITICAL）— `process()` 接收 `&self`，统计累积需要 `&mut self`。用 `Mutex` 满足 `Sync` 但热路径每条记录 lock/unlock；改 trait 签名破坏 729 个现有测试。唯一正确方案：聚合器不实现 `LogProcessor`，设计独立 `RecordVisitor` trait（`&mut self`）。

4. **并行 CSV 路径下统计数据分散**（HIGH）— `process_csv_parallel` 每个 rayon task 独立，若聚合器共享则锁竞争消除并行收益。正确方案：每 task 持有独立 `TemplateAggregator`，主线程 pool 完成后 `merge()` 合并（map-reduce 模式）。

5. **SVG 生成 BufWriter 未显式 flush 导致内容截断**（MEDIUM）— Rust `BufWriter` drop 时不自动 flush，错误被静默忽略。每个 SVG 写出函数末尾必须显式 `flush()?`。SVG 生成只能在 finalize 阶段运行，绝不出现在热循环的 `process()` 中。

## Implications for Roadmap

基于研究，建议 5 个阶段的实现顺序，严格遵循依赖链。

### Phase 1: SQL 模板归一化引擎（TMPL-01）

**Rationale:** 所有后续阶段的 key 生成依赖归一化函数；先建立函数并通过测试，后续直接调用。无外部依赖，风险最低，适合开局建立信心。

**Delivers:** `normalize_template(sql: &str) -> String` 函数；`TemplateAnalysisConfig` config struct；注释去除、IN 折叠、关键字大小写统一四项变换

**Addresses:** TMPL-01

**Avoids:** Pitfall 3（另起炉灶归一化逻辑）、Pitfall 9（大小写不一致分裂聚合）

### Phase 2: TemplateAggregator 流式统计累积器（TMPL-02）

**Rationale:** 所有输出路径（JSON 报告、SQLite 表、CSV 伴随、SVG 图表）均以 `TemplateStats` 为输入；必须先实现累积器并完成 finalize 接口，才能并行推进后续三条输出路径。风险最高——涉及 `process_log_file()` 签名变更和并行路径改造，需要最仔细的设计。

**Delivers:** `TemplateAggregator { observe(), finalize(), merge() }`；`TemplateStats` 数据结构（含 hdrhistogram 百分位）；`process_log_file()` 新增 `aggregator: Option<&mut TemplateAggregator>` 参数；并行路径 merge 策略

**Uses:** `hdrhistogram = "7.5"`（唯一新增 crate）

**Avoids:** Pitfall 1、Pitfall 2、Pitfall 5、Pitfall 12、Pitfall 13

**Research flag:** 需在 implementation plan 中明确 `RecordVisitor` trait 精确签名和 `observe()` 参数列表，避免实现中途重构。

### Phase 3: 独立 JSON/CSV 报告输出（TMPL-03）

**Rationale:** 依赖 Phase 2 的 `TemplateStats`；纯序列化逻辑，风险低；与 Phase 4、Phase 5 可并行开发。

**Delivers:** `src/report/mod.rs`；JSON 报告（新增 p50/p95/p99/histogram_buckets 字段）；CSV 报告；`first_seen`/`last_seen` 时间戳

**Uses:** `serde_json`（已有）、`itoa`/`ryu`（已有）

**Avoids:** Pitfall 7（输出时序不同步）、Pitfall 15（路径未在 validate 阶段检查）

### Phase 4: Exporter 集成（TMPL-04）

**Rationale:** 依赖 Phase 2 的 `TemplateStats`；需要与 exporter finalize 顺序严格对齐（主 exporter finalize 之后写入）。与 Phase 3 并行。

**Delivers:** `ExporterManager::write_templates()` 接口；SQLite `sql_templates` 表；CSV `*_templates.csv` 伴随文件

**Avoids:** Pitfall 7（统计写出在主 exporter finalize 前导致数据不完整）

### Phase 5: SVG 图表生成（CHART-01~05）

**Rationale:** 依赖 Phase 2 的 `TemplateStats`；与 Phase 3/4 并行，但复杂度最高（四类图表、新 crate API 验证）。放最后以确保 plotters SVG-only 功能标志行为在实现阶段再验证，不阻塞 TMPL 路径。

**Delivers:** `src/chart/mod.rs`；CHART-02（Top N 频率条形图）、CHART-03（耗时直方图）、CHART-04（时间趋势折线图）、CHART-05（用户/Schema 饼图）；`ChartsConfig` config struct

**Uses:** `plotters 0.3`（SVG-only 功能标志）

**Avoids:** Pitfall 6（SVG 生成混入热循环）、Pitfall 11（BufWriter flush 遗漏）

### Phase Ordering Rationale

- Phase 1 先行：归一化函数是 key 生成唯一来源，所有 phase 均调用它，必须先稳定接口
- Phase 2 次之：`TemplateStats` 是所有输出路径唯一数据来源；不完成 finalize 接口，Phase 3/4/5 无法开工
- Phase 3/4/5 可并行：三条输出路径相互独立，消费同一 `&TemplateStats`
- 此顺序完全避免"实现一半后发现接口不对"的大规模重构风险

### Research Flags

需要在 Phase 2 implementation plan 中明确设计（不需要额外外部研究，但需要接口决策文档）：

- **Phase 2:** `RecordVisitor` trait 精确签名；`observe()` 是否接收已归一化的 key 还是原始 sql（推荐接收 key，避免重复归一化）；CHART-04 时间分桶粒度（小时 vs 分钟）
- **Phase 5:** plotters `Pie::new()` 的 `sizes` 参数类型；长 fingerprint 标签在条形图 Y 轴的截断策略 — 建议用小型原型验证再展开实现

标准模式（可直接实现，无需 research phase）：

- **Phase 1:** 字节级 SQL 归一化，模式已在现有 `fingerprint()` 中建立
- **Phase 3:** JSON/CSV 序列化，完全复用 `DigestJson`/`itoa` 现有模式
- **Phase 4:** SQLite DDL + batch INSERT，完全复用现有 `SqliteExporter` 模式

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack（新增 crate 选型） | HIGH | hdrhistogram API 经官方文档验证；plotters SVG-only 配置经 Context7 + 代码示例验证；拒绝 sqlparser 的原因经方言列表和 sql-fingerprint 文档确认 |
| Features（功能范围） | HIGH | 基于现有 codebase 直接分析 + pt-query-digest 行业惯例；table stakes 清晰 |
| Architecture（聚合器放置方案） | HIGH | Option B（独立 struct）经现有代码 `&mut` 模式和 `LogProcessor` trait 约束直接验证；并行 merge 策略与现有 pre-scan phase merge 模式一致 |
| Pitfalls | HIGH | 全部从直接源码检查导出，非推断；Pitfall 1/2/12/13 有具体代码证据 |

**Overall confidence:** HIGH

### Gaps to Address

- **`normalize_template()` 边界情况：** 字符串字面量内部含 `--` 或 `/*` 的注释检测、嵌套括号的 IN 列表识别需要测试用例覆盖。实现中需 fuzzing 或对照测试集验证（MEDIUM confidence 区域）。

- **plotters `Pie` element 标签布局：** 长 schema 名的溢出处理行为需要 Phase 5 开始时用小型原型确认，不要等到完整实现再发现布局问题。

- **hdrhistogram vs 固定桶直方图选择：** 研究推荐 `hdrhistogram`（~24 KB/模板，误差 <2%，API 直接，`iter_recorded()` 供图表直接使用）；PITFALLS 研究提出 64 桶固定数组方案（288 bytes/模板，误差 <5%）。在 Phase 2 设计步骤中做出一次性决策并记录原因。推荐选 `hdrhistogram`。

## Sources

### Primary（HIGH confidence）

- 直接源码检查：`src/features/sql_fingerprint.rs`、`src/features/mod.rs`、`src/cli/run.rs`、`src/exporter/mod.rs`、`src/config.rs` — 所有架构决策基础
- hdrhistogram 官方文档 [https://docs.rs/hdrhistogram/latest/hdrhistogram/](https://docs.rs/hdrhistogram/latest/hdrhistogram/) — API 验证
- plotters Context7 文档 + Cargo.toml feature 标志确认 [https://docs.rs/crate/plotters/latest/source/Cargo.toml.orig](https://docs.rs/crate/plotters/latest/source/Cargo.toml.orig)

### Secondary（MEDIUM confidence）

- pt-query-digest 文档 [https://docs.percona.com/percona-toolkit/pt-query-digest.html](https://docs.percona.com/percona-toolkit/pt-query-digest.html) — table stakes 功能集参考
- sql-fingerprint 1.11.1 依赖分析 [https://docs.rs/crate/sql-fingerprint/latest/source/Cargo.toml.orig](https://docs.rs/crate/sql-fingerprint/latest/source/Cargo.toml.orig) — 拒绝原因验证
- sqlparser 0.62.0 方言列表 [https://docs.rs/sqlparser/latest/sqlparser/](https://docs.rs/sqlparser/latest/sqlparser/) — 无 DaMeng 方言验证

---
*Research completed: 2026-05-15*
*Ready for roadmap: yes*

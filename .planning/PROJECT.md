# sqllog2db — 达梦 SQL 日志解析工具

## Current State: v1.3 已发布 ✅ (2026-05-17)

sqllog2db 完成四个里程碑迭代，现具备完整的 SQL 模板分析与 SVG 可视化能力。`normalize_template()` 归一化引擎、`TemplateAggregator` 流式统计（hdrhistogram）、双路统计输出（SQLite 表 + CSV 伴随文件）、四类 SVG 图表全部上线。418 项测试通过，热循环快路径无回归。

## Next Milestone Goals (v1.4 — TBD)

- TMPL-03/03b：独立 JSON/CSV 报告输出（DBA 可读）
- Tech debt：补全 VERIFICATION.md（Phases 12/13/14/16）+ Nyquist 合规性
- 可能方向：交互式过滤扩展、性能进一步提升

---

## Previous Milestones

- ✅ **v1.3** (2026-05-17) — SQL 模板分析 & 可视化（Phases 12–16）
- ✅ **v1.2** (2026-05-15) — 质量强化 & 性能深化（Phases 7–11）
- ✅ **v1.1** (2026-05-10) — 性能优化（Phases 3–6）
- ✅ **v1.0** (2026-04-18) — 增强 SQL 内容过滤与字段投影（Phases 1–2）

sqllog2db 已完成三个里程碑的迭代，具备完整的过滤与导出能力，性能基础设施健全，代码质量与可审计性达到当前合理上限。

## What This Is

sqllog2db 是一个用于解析达梦数据库 SQL 日志文件并将其导出为 CSV 或 SQLite 的命令行工具。以流式方式处理日志记录，通过可选的 Pipeline 过滤器处理后写入配置的导出器。支持正则表达式多字段过滤（AND 语义 include + OR-veto exclude）和输出字段精确控制。

## Core Value

用户能够精确指定"导出哪些记录的哪些字段"——过滤逻辑清晰可配置，输出结果完全可控。

## Requirements

### Validated

- ✓ 流式解析达梦 SQL 日志文件 — existing
- ✓ 导出到 CSV 和 SQLite — existing
- ✓ Pipeline 过滤器（记录级 + 事务级） — existing
- ✓ 字段投影（ordered_indices Vec） — existing
- ✓ 参数归一化 / SQL 指纹 — existing
- ✓ 增量断点续传（resume state） — existing
- ✓ 并行 CSV 处理（rayon） — existing
- ✓ **FILTER-01**: 对任意字段支持正则表达式匹配过滤 — Phase 1
- ✓ **FILTER-02**: 多关键词列表默认 AND 语义（全部满足才保留） — Phase 1
- ✓ **FIELD-01**: 输出字段控制——用户可在 config 中指定导出哪些字段 — Phase 2
- ✓ **PERF-01**: profile CSV 和 SQLite 热路径，生成 flamegraph/criterion 报告 — Phase 3
- ✓ **PERF-02**: CSV 写入吞吐优化（accept-defer：合成 -8.53%；上游解析层留 Phase 6）— Phase 4
- ✓ **PERF-03**: CSV 格式化路径优化（criterion micro-benchmark 验证）— Phase 4
- ✓ **PERF-04**: SQLite 批量事务 + prepared statement 复用 — Phase 5
- ✓ **PERF-06**: SQLite prepared statement 复用 — Phase 5
- ✓ **PERF-07**: dm-database-parser-sqllog 1.0.0 评估完成（index() 不集成，改进自动生效）— Phase 6
- ✓ **PERF-08**: 热循环堆分配减少（条件 reserve + include_pm 兜底）— Phase 4
- ✓ **PERF-09**: 651 测试全部通过，0 失败 — Phase 6
- ✓ **DEBT-01**: SQLite 静默错误显式化——handle_delete_clear_result() 区分软失败与真实错误 — Phase 7
- ✓ **DEBT-02**: table_name SQL 注入防护——ASCII 白名单校验 + 5 处 DDL 双引号转义 — Phase 7
- ✓ **DEBT-03**: Nyquist Phase 3/4/5/6 VALIDATION.md 补签——Nyquist 审计链全段闭合 — Phase 11
- ✓ **FILTER-03**: 排除过滤器（exclude filters）——7 个 exclude_* 字段，OR-veto 语义，pipeline 快路径不受影响 — Phase 8
- ✓ **PERF-10**: samply + criterion 量化热路径，D-G1 门控未触发（最高 src/ 函数 4.6% < 5%），"已达当前瓶颈"签署 — Phase 10
- ✓ **PERF-11**: validate_and_compile() 统一接口消除双重 regex 编译；update check 后台化；hyperfine 基线记录 — Phase 9

  - ✓ **TMPL-01**: 用户可通过 config 启用 SQL 模板归一化（注释去除、IN 折叠、关键字大写、字面量保护）— Phase 12
  - ✓ **TMPL-02**: 用户可启用模板统计聚合器，流式累积 count/avg/min/max/p50/p95/p99，hdrhistogram ~24KB/模板 — Phase 13
  - ✓ **TMPL-04**: SQLite 导出时生成 `sql_templates` 统计表；CSV 导出时生成 `*_templates.csv` 伴随文件 — Phase 14
  - ✓ **CHART-01**: 用户可在 config 中指定 SVG 输出目录，运行后自动生成图表文件 — Phase 15
  - ✓ **CHART-02**: 生成 Top N 模板执行频率横向条形图（SVG） — Phase 15
  - ✓ **CHART-03**: 生成全局耗时分布直方图（SVG，对数轴，hdrhistogram bucket） — Phase 15
  - ✓ **CHART-04**: 生成 SQL 执行频率时间趋势折线图（SVG，小时粒度） — Phase 16
  - ✓ **CHART-05**: 生成用户 / Schema 执行占比饼图（SVG，HSL 颜色，Others 聚合） — Phase 16

### Active

- [ ] **TMPL-03**: 模板统计结果输出为独立 JSON 报告文件（config 指定路径）— 延后至 v1.4+
- [ ] **TMPL-03b**: 模板统计结果输出为独立 CSV 摘要文件（DBA 可用 Excel 打开）— 延后至 v1.4+

### Out of Scope

- OR 条件组合（FILTER-04）— 简单列表 AND 已满足需求，OR 增加配置复杂度
- 跨字段联合条件（FILTER-05）— 暂不支持"字段A 满足 X 且 字段B 满足 Y"的复合谓词
- 运行时动态修改过滤规则 — 配置在启动时加载，不支持热重载
- `exclude_trxids` 正则支持 — 保持与 include_trxids 的 HashSet 精确匹配对称性
- SQLite WAL 模式 — 用户决策移除：数据无需崩溃保护（v1.1）
- JSON / Parquet 导出 — 超出当前里程碑范围

## Context

- 架构：过滤层（`src/features/filters.rs`）+ 模板分析层（`src/features/template_aggregator.rs` + `sql_fingerprint.rs`）+ 图表层（`src/charts/`）
- `FilterProcessor` 热路径使用 `CompiledMetaFilters` + `CompiledSqlFilters`（预编译，启动时 validate_and_compile()）
- `TemplateAggregator` 通过 `Option<&mut TemplateAggregator>` 侧路径接入热循环，不实现 `LogProcessor`
- `ordered_indices: Vec<usize>` 注入到 `CsvExporter` / `SqliteExporter`，支持任意字段顺序投影
- 热循环中 `pipeline.is_empty()` 保证无过滤时零开销（未改动）
- 图表生成在 `finalize()` 之前，保证 D-01 约束（plotters SVG-only，无字体/图像依赖）
- Rust LOC: ~14,164 | 测试套件: 418 tests | 基准: ~5.2M records/sec (CSV synthetic)
- Tech debt: VERIFICATION.md 缺失（Phases 12/13/14/16），Nyquist 合规性补签待完成

## Constraints

- **性能**: 过滤逻辑不能破坏热循环的零开销快路径（pipeline.is_empty() 检查）
- **配置格式**: 使用 TOML，与现有 `config.toml` 风格保持一致
- **兼容性**: 不改变现有无过滤配置的行为
- **函数长度**: ≤ 40 行（CLAUDE.md 约束，v1.2 全量满足）

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| 列表默认 AND 语义 | 简单直观，覆盖最常见的"同时包含多个关键词"场景 | ✓ Phase 1 实现 |
| 对任意字段过滤（非仅 sql_text） | 用户需求：按 user/schema/ip 等字段过滤 | ✓ Phase 1 实现 |
| ordered_indices Vec 替代 FieldMask 投影 | 支持任意字段顺序，FieldMask 只能全部/按默认顺序输出 | ✓ Phase 2 实现 |
| FILTER-03 集成进 CompiledMetaFilters | 避免独立 ExcludeProcessor 双调用开销，排除先于包含检查短路更快 | ✓ Phase 8 |
| PERF-11 门控：hyperfine >50ms 才后台化 update check | 数据驱动，避免过度工程 | ✓ Phase 9 |
| validate_and_compile() 合并接口 | 单次编译结果贯穿全链路，消除双重 Regex::new() | ✓ Phase 9 |
| PERF-10 D-G1 门控：>5% 才优化 | 避免盲目优化，与 v1.1 策略一致 | ✓ Phase 10（未命中，记录结论）|
| SQLite WAL 模式移除 | 数据无需崩溃保护，WAL checkpointing 开销得不偿失 | ✓ Phase 5 |
| TemplateAggregator 不实现 LogProcessor trait | `process()` 接收 `&self`，累积需 `&mut self`；加入 Pipeline 破坏快路径 | ✓ Phase 13 |
| hdrhistogram 存储耗时样本 | Vec<u64> 单热模板 5M 记录 ~40MB；hdrhistogram ~24KB，误差 <2% | ✓ Phase 13 |
| plotters SVG-only 配置 | 无字体/图像系统依赖；charts-rs（字体）和 charming（JS）均排除 | ✓ Phase 15 |
| observe() 接收已归一化 key | 避免 TemplateAggregator 内部重复归一化，稳定性由 Phase 12 保证 | ✓ Phase 13 |
| 并行 CSV map-reduce merge() | 每 rayon task 独立 TemplateAggregator，主线程合并，消除锁竞争 | ✓ Phase 13 |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-05-17 after v1.3 milestone*

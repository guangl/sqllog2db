# Roadmap: sqllog2db

## Milestones

- ✅ **v1.0 增强 SQL 内容过滤与字段投影** — Phases 1–2 (shipped 2026-04-18)
- ✅ **v1.1 性能优化** — Phases 3–6 (shipped 2026-05-10)
- ✅ **v1.2 质量强化 & 性能深化** — Phases 7–11 (shipped 2026-05-15)
- 🚧 **v1.3 SQL 模板分析 & 可视化** — Phases 12–16 (active)

## Phases

<details>
<summary>✅ v1.0 增强 SQL 内容过滤与字段投影 (Phases 1–2) — SHIPPED 2026-04-18</summary>

- [x] Phase 1: 正则字段过滤 (2/2 plans) — completed 2026-04-18
- [x] Phase 2: 输出字段控制 (4/4 plans) — completed 2026-04-18

Full details: `.planning/milestones/v1.0-ROADMAP.md`

</details>

<details>
<summary>✅ v1.1 性能优化 (Phases 3–6) — SHIPPED 2026-05-10</summary>

- [x] Phase 3: Profiling & Benchmarking (3/3 plans) — completed 2026-04-27
- [x] Phase 4: CSV 性能优化 (4/4 plans) — completed 2026-05-09
- [x] Phase 5: SQLite 性能优化 (3/3 plans) — completed 2026-05-10
- [x] Phase 6: 解析库集成 + 验收 (2/2 plans) — completed 2026-05-10

Full details: `.planning/milestones/v1.1-ROADMAP.md`

</details>

<details>
<summary>✅ v1.2 质量强化 & 性能深化 (Phases 7–11) — SHIPPED 2026-05-15</summary>

- [x] Phase 7: 技术债修复 (1/1 plans) — completed 2026-05-10
- [x] Phase 8: 排除过滤器 (2/2 plans) — completed 2026-05-10
- [x] Phase 9: CLI 启动提速 (5/5 plans) — completed 2026-05-14
- [x] Phase 10: 热路径优化 (3/3 plans) — completed 2026-05-15
- [x] Phase 11: Nyquist 补签 (2/2 plans) — completed 2026-05-15

Full details: `.planning/milestones/v1.2-ROADMAP.md`

</details>

### v1.3 SQL 模板分析 & 可视化

- [ ] **Phase 12: SQL 模板归一化引擎** — normalize_template() 函数 + TemplateAnalysisConfig
- [ ] **Phase 13: TemplateAggregator 流式统计累积器** — 侧路径聚合 + hdrhistogram 百分位
- [ ] **Phase 14: Exporter 集成输出** — sql_templates 表 + *_templates.csv 伴随文件
- [ ] **Phase 15: SVG 图表基础设施 + 前两类图表** — ChartsConfig + Top N 频率条形图 + 耗时直方图
- [ ] **Phase 16: 剩余图表** — 时间趋势折线图 + 用户/Schema 饼图

## Phase Details

### Phase 12: SQL 模板归一化引擎
**Goal**: 用户可通过 config 启用 SQL 模板归一化，`normalize_template()` 对 sql_text 执行四项变换并生成稳定的模板 key
**Depends on**: Nothing (first v1.3 phase)
**Requirements**: TMPL-01
**Success Criteria** (what must be TRUE):
  1. 用户在 config 中设置 `[features.template_analysis] enabled = true` 后，运行时每条记录的 sql_text 经过注释去除、IN 列表折叠、关键字大小写统一、多余空白规范化四项变换
  2. 相同语义但空白/大小写/IN 列表长度不同的 SQL 语句经 normalize_template() 后产生相同的模板 key
  3. 字符串字面量内部的注释符号（`--`、`/*`）不被误判为注释，归一化结果保留字面量原文
  4. `cargo test` 中有针对四项变换边界情况的单元测试，全部通过
**Plans**: 3 plans
- [ ] 12-01-PLAN.md — 在 src/features/sql_fingerprint.rs 实现 normalize_template() 函数（共享扫描基础 + 四项变换 + 7 项行为子测试）
- [ ] 12-02-PLAN.md — 新增 TemplateAnalysisConfig + FeaturesConfig 接入 + pub use 导出 + init 命令 TOML 模板段
- [ ] 12-03-PLAN.md — 在 cli/run.rs 热循环接入 do_template 守卫 + process_log_file/process_csv_parallel 签名透传 + 集成测试

### Phase 13: TemplateAggregator 流式统计累积器
**Goal**: 用户可启用模板统计聚合，run 结束后每个模板输出 count + avg/min/max + p50/p95/p99 + first_seen/last_seen，且热循环零开销快路径完全不受影响
**Depends on**: Phase 12 (normalize_template())
**Requirements**: TMPL-02
**Success Criteria** (what must be TRUE):
  1. `TemplateAggregator` 不实现 `LogProcessor` trait，通过 `Option<&mut TemplateAggregator>` 侧路径接入 `process_log_file()`，禁用统计时 `pipeline.is_empty()` 快路径行为与 v1.2 完全一致
  2. 每个模板使用 `hdrhistogram::Histogram<u64>` 存储耗时样本（~24 KB/模板），5M 记录规模下内存占用不随记录数线性增长
  3. `finalize()` 调用后每个模板输出包含 count、avg、min、max、p50、p95、p99、first_seen、last_seen 的 `TemplateStats` 结构
  4. 并行 CSV 路径中每个 rayon task 持有独立 `TemplateAggregator`，主线程通过 `merge()` 合并，统计结果与单线程路径一致
  5. `cargo clippy --all-targets -- -D warnings` 通过，`cargo test` 新增针对 observe/finalize/merge 的单元测试全部通过
**Plans**: 2 plans
- [ ] 13-01-PLAN.md — 新建 src/features/template_aggregator.rs（TemplateEntry + TemplateAggregator + TemplateStats + 6 项单元测试）+ Cargo.toml hdrhistogram 依赖 + mod.rs 暴露 + sql_fingerprint.rs allow(dead_code) 清理
- [ ] 13-02-PLAN.md — src/cli/run.rs 侧路径接入：process_log_file 签名替换 + 热循环 observe 调用 + process_csv_parallel map-reduce + handle_run 创建/finalize + 2 项集成测试

### Phase 14: Exporter 集成输出
**Goal**: SQLite 导出时自动写入 sql_templates 统计表，CSV 导出时自动生成 *_templates.csv 伴随文件，统计数据与主导出结果保持一致
**Depends on**: Phase 13 (TemplateStats)
**Requirements**: TMPL-04
**Success Criteria** (what must be TRUE):
  1. SQLite 导出模式下，run 结束后数据库文件中存在 `sql_templates` 表，包含每个模板的 template_key、count、avg_us、min_us、max_us、p50_us、p95_us、p99_us、first_seen、last_seen 列
  2. CSV 导出模式下，run 结束后在 CSV 文件同目录生成 `<basename>_templates.csv` 伴随文件，列结构与 SQLite 表一致
  3. `sql_templates` 写入在主 exporter `finalize()` 之后执行，任何主导出提前终止时伴随文件不被写出（数据完整性保证）
  4. 未启用模板统计（`template_analysis.enabled = false`）时，SQLite 数据库不创建 `sql_templates` 表，CSV 目录不生成伴随文件
**Plans**: TBD
**UI hint**: no

### Phase 15: SVG 图表基础设施 + 前两类图表
**Goal**: 用户可在 config 中配置 SVG 输出目录，run 结束后自动生成 Top N 模板频率条形图和全局耗时分布直方图两类 SVG 文件
**Depends on**: Phase 13 (TemplateStats)
**Requirements**: CHART-01, CHART-02, CHART-03
**Success Criteria** (what must be TRUE):
  1. 用户在 config 中设置 `[features.charts] output_dir = "charts/"` 后，run 结束时 `charts/` 目录下出现 SVG 文件，文件可被浏览器直接打开且图表元素完整渲染
  2. `top_n_frequency.svg` 包含横向条形图，每条对应一个模板（按执行频率降序排列），Y 轴标签截断超长 fingerprint，`n` 值可在 config 中配置
  3. `latency_histogram.svg` 包含耗时分布直方图，使用 hdrhistogram `iter_recorded()` bucket 数据作为输入，X 轴为耗时区间，Y 轴为记录数
  4. 每个 SVG 写出函数在关闭前显式调用 `flush()?`，文件内容完整无截断
  5. 未启用图表功能时，不创建 `output_dir` 目录，不生成任何 SVG 文件
**Plans**: TBD
**UI hint**: yes

### Phase 16: 剩余图表
**Goal**: 用户在已有图表基础上获得时间趋势折线图和用户/Schema 占比饼图，完整覆盖 v1.3 全部可视化需求
**Depends on**: Phase 15 (chart infrastructure, plotters SVG backend)
**Requirements**: CHART-04, CHART-05
**Success Criteria** (what must be TRUE):
  1. `frequency_trend.svg` 包含时间趋势折线图，X 轴为小时粒度时间戳，Y 轴为该小时内 SQL 执行总次数，折线连续且时间轴标签可读
  2. `user_schema_pie.svg` 包含用户/Schema 执行占比饼图，每个扇区对应一个用户或 Schema，标签与扇区颜色对应，长名称有截断处理
  3. 两类图表仅在 finalize 阶段生成，不出现在热循环 `process()` 路径中
  4. `cargo clippy --all-targets -- -D warnings` 通过，`cargo test` 全部通过
**Plans**: TBD
**UI hint**: yes

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. 正则字段过滤 | v1.0 | 2/2 | Complete | 2026-04-18 |
| 2. 输出字段控制 | v1.0 | 4/4 | Complete | 2026-04-18 |
| 3. Profiling & Benchmarking | v1.1 | 3/3 | Complete | 2026-04-27 |
| 4. CSV 性能优化 | v1.1 | 4/4 | Complete | 2026-05-09 |
| 5. SQLite 性能优化 | v1.1 | 3/3 | Complete | 2026-05-10 |
| 6. 解析库集成 + 验收 | v1.1 | 2/2 | Complete | 2026-05-10 |
| 7. 技术债修复 | v1.2 | 1/1 | Complete | 2026-05-10 |
| 8. 排除过滤器 | v1.2 | 2/2 | Complete | 2026-05-10 |
| 9. CLI 启动提速 | v1.2 | 5/5 | Complete | 2026-05-14 |
| 10. 热路径优化 | v1.2 | 3/3 | Complete | 2026-05-15 |
| 11. Nyquist 补签 | v1.2 | 2/2 | Complete | 2026-05-15 |
| 12. SQL 模板归一化引擎 | v1.3 | 0/3 | Planned | - |
| 13. TemplateAggregator 流式统计累积器 | v1.3 | 0/2 | Planned | - |
| 14. Exporter 集成输出 | v1.3 | 0/? | Not started | - |
| 15. SVG 图表基础设施 + 前两类图表 | v1.3 | 0/? | Not started | - |
| 16. 剩余图表 | v1.3 | 0/? | Not started | - |

# Requirements: sqllog2db v1.3

**Defined:** 2026-05-15
**Core Value:** 用户能够精确指定"导出哪些记录的哪些字段"——过滤逻辑清晰可配置，输出结果完全可控

## v1.3 Requirements

### SQL 模板分析（TMPL）

- [ ] **TMPL-01**: 用户可在 config 中启用模板归一化，`normalize_template()` 在 `replace_parameters` 之后对 sql_text 执行注释去除、IN 列表折叠、关键字大小写统一，生成稳定的模板 key
- [x] **TMPL-02**: 用户可启用模板统计聚合，run 结束后每个模板输出 count + avg/min/max + p50/p95/p99 + first_seen/last_seen；使用 hdrhistogram（~24 KB/模板），禁止 Vec 全量样本存储
- [ ] **TMPL-04**: SQLite 导出时自动写入 `sql_templates` 统计表；CSV 导出时自动生成 `*_templates.csv` 伴随文件

### SVG 图表生成（CHART）

- [ ] **CHART-01**: 用户可在 config 的 `[features.charts]` 中指定 `output_dir`，启用后 run 结束时自动生成 SVG 文件到该目录
- [ ] **CHART-02**: 生成 Top N 模板执行频率横向条形图（SVG），N 可在 config 中配置
- [ ] **CHART-03**: 生成全局耗时分布直方图（SVG），使用 hdrhistogram bucket 数据
- [ ] **CHART-04**: 生成 SQL 执行频率时间趋势折线图（SVG），小时粒度
- [ ] **CHART-05**: 生成用户 / Schema 执行占比饼图（SVG）

## Future Requirements

### 报告输出（延后至 v1.4+）

- **TMPL-03**: 用户可指定独立 JSON 报告输出路径，run 结束后生成包含 p50/p95/p99/histogram_buckets 的机器可读报告
- **TMPL-03b**: 用户可指定独立 CSV 报告输出路径，run 结束后生成 DBA 可直接用 Excel 打开的模板统计摘要

## Out of Scope

| Feature | Reason |
|---------|--------|
| 交互式 HTML 仪表盘 | 需要 JS 运行时，破坏静态文件约束 |
| 精确 p50/p95/p99（Vec 全量存储） | 内存不可控；hdrhistogram 误差 <2% 对 DBA 已足够 |
| 模板相似度聚类 | O(N²) 成本，超出当前里程碑范围 |
| OR 条件组合（FILTER-04） | 继承自 v1.1 Out of Scope 决策 |
| 跨字段联合条件（FILTER-05） | 继承自 v1.1 Out of Scope 决策 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TMPL-01 | Phase 12 | Pending |
| TMPL-02 | Phase 13 | Complete |
| TMPL-04 | Phase 14 | Pending |
| CHART-01 | Phase 15 | Pending |
| CHART-02 | Phase 15 | Pending |
| CHART-03 | Phase 15 | Pending |
| CHART-04 | Phase 16 | Pending |
| CHART-05 | Phase 16 | Pending |

**Coverage:**
- v1.3 requirements: 8 total
- Mapped to phases: 8 ✓
- Unmapped: 0 ✓

---
*Requirements defined: 2026-05-15*
*Last updated: 2026-05-15 — Phase 12–16 mapped (v1.3 roadmap created)*

# Retrospective

## Milestone: v1.3 — SQL 模板分析 & 可视化

**Shipped:** 2026-05-17
**Phases:** 5 | **Plans:** 19

### What Was Built

- `normalize_template()` 共享扫描引擎（`ScanMode` enum 复用 `fingerprint()` 基础设施）——注释去除、IN 折叠、关键字大写、字面量保护四项变换
- `TemplateAggregator` 侧路径聚合——`Option<&mut TemplateAggregator>` 绑路，hdrhistogram ~24KB/模板，rayon map-reduce merge，`pipeline.is_empty()` 快路径零影响
- 双路统计输出——SQLite `sql_templates` 表（单事务 INSERT）+ CSV `*_templates.csv` 伴随文件（itoa 零分配）
- SVG 图表基础设施——plotters SVG-only，Top N 频率横向条形图 + 对数轴耗时直方图
- 时间趋势折线图（`hour_counts` BTreeMap 小时桶）+ 用户/Schema 饼图（`user_counts` AHashMap + HSL 颜色生成）

### What Worked

- **侧路径设计（Option<&mut T>）** — 避免了 `TemplateAggregator` 实现 `LogProcessor` 的架构困难（`process()` 要求 `&self`），直接解决了可变性冲突
- **ScanMode 枚举复用扫描引擎** — Phase 12 复用 `fingerprint()` 的扫描状态机，避免了维护两套几乎相同的 SQL 扫描代码
- **骨架 + `#[allow(dead_code)]` 渐进接入** — Phase 14/15 的骨架阶段先用 `#[allow(dead_code)]` 占位，后续 Plan 接入后自动消除；与 v1.0/v1.2 相同的成熟模式
- **plotters SVG-only 约束** — 从一开始排除 bitmap 后端，避免了字体/图像系统依赖，跨平台构建干净

### What Was Inefficient

- **ROADMAP.md 进度表陈旧** — Phase 12 实际 3/3 完成但 ROADMAP 显示 "0/3 Planned"，Phase 15 显示 "2/5" 但实际 5/5——进度表未同步执行状态
- **VERIFICATION.md 文档缺失** — 4 个 phases（12/13/14/16）缺少 VERIFICATION.md，导致审计 `gaps_found`；代码已验证但文档记录滞后
- **REQUIREMENTS.md traceability 表格过期** — CHART-01~05 全部显示 Pending，实际已完成；计划阶段写入后未同步更新

### Patterns Established

- `Option<&mut TemplateAggregator>` 侧路径——安全、零额外 trait 实现、热路径隔离的可变聚合器接入模式
- SVG 图表模块结构：`src/charts/mod.rs` (generate_charts dispatch) + 各图表独立文件（frequency_bar/latency_hist/trend_line/user_pie）
- `draw_*` 函数规范：接收借用数据 + 路径 + `top_n`，显式调用 `root.present()?`（flush 保证）
- BTreeMap 时间桶（前 13 字符 `YYYY-MM-DD HH`）——有序遍历免排序，单日/多日标签格式自动切换

### Key Lessons

- 侧路径聚合优于 trait 实现：当聚合器需要可变性而 trait 方法是不可变引用时，`Option<&mut T>` 参数是比 `Mutex<T>` 更简洁的解决方案
- 文档债务应与代码同步——VERIFICATION.md 在 plan 完成时就应创建，而非留到里程碑关闭时补签
- plotters SVG 对数轴需要手动离散化 X 轴标签（`iter_recorded()` bucket 值不均匀），这不是 API 缺陷而是数据特性

### Cost Observations

- Timeline: 3 days (2026-05-15 → 2026-05-17)
- Commits: ~102 since v1.2
- Notable: Phase 16 review cycle（16-REVIEW.md → 16-REVIEW-FIX.md）在同一天内完成，SVG 渲染问题（黑色遮层、canvas 高度、数值溢出）均在首轮 review 中发现并修复

---

## Milestone: v1.0 — 增强 SQL 内容过滤与字段投影

**Shipped:** 2026-04-18
**Phases:** 2 | **Plans:** 6

### What Was Built

- Pre-compiled regex filter structs (`CompiledMetaFilters` + `CompiledSqlFilters`) with AND cross-field / OR intra-field semantics, startup validation
- `FilterProcessor` hot path integrated with compiled regex on all 7 meta fields
- `FeaturesConfig::ordered_field_indices()` for user-specified field order projection
- `CsvExporter` + `SqliteExporter` extended with `ordered_indices` — full field projection pipeline
- End-to-end wiring through `ExporterManager` and parallel CSV path

### What Worked

- **TDD RED/GREEN pattern** — writing failing tests first caught interface design issues early (Plan 01-01)
- **Pre-compile at startup** strategy — moving regex compilation to startup (not hot loop) kept the performance guarantee simple to reason about
- **`#[allow(dead_code)]` staging** — marking new structs as dead_code in Plan 01, removing in Plan 02 made the two-plan dependency explicit and clean
- **Atomic plan commits** — each plan had a clean, reviewable commit; deviations (clippy fixes) were folded in without scope creep

### What Was Inefficient

- REQUIREMENTS.md checkboxes were never updated during phase execution — required manual acknowledgement at milestone close
- STATE.md Performance Metrics section was left with placeholder dashes throughout the milestone (not auto-populated)

### Patterns Established

- `ordered_indices: Vec<usize>` as the projection API — cleaner than FieldMask bitmask for arbitrary ordering
- Reference-based construction (`FilterProcessor::new(&FiltersFeature)`) avoids clippy `needless_pass_by_value` from the start
- Re-export compiled types via `features::mod` for a clean public API boundary

### Key Lessons

- Clippy `-D warnings` catches interface design issues (pass-by-value, dead_code, must_use) that are cheaper to fix during the plan than after
- Two-plan structure (core structs → hot path wiring) worked well for regex feature: Plan 01 was pure logic, Plan 02 was pure integration — no mixing
- `ordered_indices` replacing FieldMask was the right call: the FieldMask approach would have required separate ordering metadata anyway

### Cost Observations

- Sessions: single-day execution (2026-04-18)
- Notable: all 6 plans executed sequentially in one session with no context resets required

---

## Milestone: v1.1 — 性能优化

**Shipped:** 2026-05-10
**Phases:** 4 | **Plans:** 12

### What Was Built

- Flamegraph + criterion benchmark 基础设施（CSV/SQLite 双路径，real-file + synthetic）
- CSV 条件 reserve + `include_performance_metrics` 配置项，热循环分配减少
- SQLite `batch_commit_if_needed()` 批量事务（5x 性能差距）+ `prepare_cached()` statement 复用
- dm-database-parser-sqllog 1.0.0 升级 + PERF-07 API 评估存档
- 651 测试全部通过，clippy 零警告

### What Worked

- **Profile-first approach** — Phase 3 先用 flamegraph 定位热路径，避免了在 CSV 格式化层（只占 ~5%）投入过多时间；真正的热点（parse_meta/memmove）在上游 crate，Phase 6 通过升级自动获益
- **accept-defer 机制** — PERF-02 real-file 数据无法在 agent 环境采集，用户明确 accept-defer，避免了阻塞整个 milestone
- **用户决策快速关闭** — WAL 模式（PERF-05）实测超 hard limit，用户当场决策移除，ROADMAP 即时更新，无返工
- **Wave 设计** — Phase 5 三波次（config → 实现 → benchmark）解耦得干净，每波次可独立验证
- **parallel csv + sqlite paths** — Phase 4 和 Phase 5 并行规划，无依赖冲突，节省时间

### What Was Inefficient

- **Nyquist VALIDATION.md 停留在 draft** — 4 个 phase 的 VALIDATION.md 均未更新为 compliant，作为文档债务带入 v1.2
- **06-02-PLAN.md Task 2/3 未执行** — ROADMAP 和 REQUIREMENTS 状态更新被 orchestrator 跳过，需要 Phase 6 验收后人工确认（实际在 VERIFICATION.md 的 Human Verification 中处理）
- **SUMMARY frontmatter requirements-completed 缺字段** — Phase 6 两个 SUMMARY 缺少该字段，3-source 交叉验证只有 2/3 sources

### Patterns Established

- `batch_commit_if_needed()` 模式 — 每 N 行提交一次，`row_count % batch_size == 0` 判断，简单有效
- `prepare_cached()` 替代 `prepare()` — rusqlite StatementCache LRU，对所有 export 路径统一应用
- CI-safe benchmark skip — `if !real_dir.exists() { eprintln!(...); return; }` 模式，保证 CI 不 panic
- Phase-level accept-defer — 在 VERIFICATION.md frontmatter 记录 override 和 accepted_by，形成审计轨迹

### Key Lessons

- 性能优化前必须 profile：假设 CSV 格式化是瓶颈是错的，真正的热路径在上游 crate
- hard limit = median × 1.05 是一个好的容差设计，既有弹性又有约束力
- WAL 模式不一定比 journal_mode=OFF 快——在写入密集场景下，WAL 的 checkpointing 开销可能反而更高
- accept-defer 要在 VERIFICATION.md 中留有迹可查，否则后续 audit 会困惑

### Cost Observations

- Sessions: 多 session 执行（2026-04-26 → 2026-05-10，14 天）
- Notable: Phase 5 WAL 实现→回退产生了额外工作（feat→fix），但用户决策快速，总体无阻塞

---

---

## Milestone: v1.2 — 质量强化 & 性能深化

**Shipped:** 2026-05-15
**Phases:** 5 (7–11) | **Plans:** 13

### What Was Built

- SQLite 双重技术债修复：`handle_delete_clear_result()` 软失败区分 + ASCII 白名单校验 + DDL 双引号转义（DEBT-01/02）
- 排除过滤器 FILTER-03：7 个 `exclude_*` 字段 OR-veto 语义，21 个新测试，快路径零开销
- `validate_and_compile()` 统一接口：消除双重 regex 编译，update check 后台化（PERF-11）
- 热路径 D-G1 门控：samply 数据驱动，4.6% < 5%，"已达当前瓶颈"签署（PERF-10）
- Nyquist 审计链闭合：Phase 3/4/5/6 VALIDATION.md 全部补签（DEBT-03）

### What Worked

- **D-G1 门控设计** — ">5% 可消除热点才优化"规则有效避免了无依据优化，samply 数据直接作为决策依据，执行简洁
- **FILTER-03 集成位置决策** — 将 exclude 集成进 CompiledMetaFilters 而非独立 processor，短路语义（exclude 先于 include 检查）带来性能优势，同时保持 pipeline.is_empty() 快路径
- **Phase 11 纯文档排最后** — DEBT-03 是纯文档补签，无代码依赖，排在最后不阻塞任何功能交付，执行极快（~15min + ~2min）
- **validate_and_compile() 接口设计** — 单次编译结果 `Option<(Meta, Sql)>` 的传递类型简洁，贯穿 handle_run → build_pipeline → FilterProcessor 全链路
- **快路径不受影响验证** — Phase 8 明确测试了空 exclude 配置下 pipeline.is_empty() 行为，避免性能回归担忧

### What Was Inefficient

- **REQUIREMENTS.md 追踪脱节** — Phase 7/8 执行期间 REQUIREMENTS.md 的 checkbox 未同步更新（DEBT-01/02/03/FILTER-03 仍显示 [ ]），在里程碑关闭时需人工核实实际状态。这是 v1.0/v1.1 已知问题，v1.2 仍未解决
- **ROADMAP.md Progress 表未即时更新** — Phase 7/8 完成后 Progress 表仍显示"0/1 Not started"，在里程碑关闭时才修正
- **Phase 9 需要 5 个 plan** — Wave 4 (09-05) 是 gap closure，说明 09-01~04 的 SC-2 验证 BLOCKER 在规划阶段未被充分预见

### Patterns Established

- `validate_and_compile()` 模式：校验与编译合并为单次操作，结果从入口贯穿至消费点，可作为未来 config 扩展的参考
- D-G1 门控签署模式：BENCHMARKS.md Phase N 节以 §D-G1 门控判定 + §当前瓶颈分析 记录，形成可审计的优化决策轨迹
- WAL N/A 注释格式：VALIDATION.md 中 `[N/A] ... *N/A — PERF-xx canceled ...*` 保留决策历史而不阻塞 compliant 状态

### Key Lessons

1. 性能优化门控应在 discuss 阶段就明确量化阈值（>5%），避免执行时主观判断
2. REQUIREMENTS.md checkbox 的追踪脱节是系统性问题——在 executor 工作流中缺乏自动同步机制
3. 纯文档型 phase（如 Nyquist 补签）执行成本极低，可安全排在最后，但在里程碑规划时应明确标记为"纯文档"
4. 技术债如果有明确的 phase 承接（DEBT-01/02 → Phase 7），就算追踪文件脱节也不会丢失——SUMMARY.md 是可靠的完成证据

### Cost Observations

- Sessions: 5 天（2026-05-10 → 2026-05-15）
- Notable: Phase 11 两个 plan 总耗时约 17 分钟，是里程碑中执行最快的 phase

---

## Cross-Milestone Trends

| Metric | v1.0 | v1.1 | v1.2 |
|--------|------|------|------|
| Phases | 2 | 4 | 5 |
| Plans | 6 | 12 | 13 |
| Days | 1 | 14 | 5 |
| Auto-fixed deviations | 6 (all clippy) | 1 (WAL revert) | 3 (09-05 gap closure, 10 review fixes) |
| Scope creep | 0 | 0 | 0 |
| Test suite at close | 629+ | 673 | 729 |
| Accept-defer decisions | 0 | 1 (PERF-02 real-file) | 0 |
| User scope removals | 0 | 1 (PERF-05 WAL) | 0 |
| Gate-driven decisions | 0 | 0 | 1 (D-G1 B-no, Phase 10) |

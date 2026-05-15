# Phase 14: Exporter 集成输出 - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning

<domain>
## Phase Boundary

实现 `Exporter` trait 新方法 `write_template_stats()`，使 SQLite 导出模式在 `finalize()` 之后写入 `sql_templates` 统计表，CSV 导出模式在 `finalize()` 之后生成 `<basename>_templates.csv` 伴随文件。两条路径（顺序/并行）均通过 `ExporterManager::write_template_stats()` 统一调用。

本阶段 **不涉及** 图表生成（Phase 15/16）或独立 JSON/CSV 报告（Future v1.4+）。

</domain>

<decisions>
## Implementation Decisions

### 写入集成点设计

- **D-01:** `Exporter` trait 新增方法 `write_template_stats(&mut self, stats: &[TemplateStats], final_path: Option<&Path>) -> Result<()>`；`SqliteExporter`、`CsvExporter`、`DryRunExporter` 各自实现；`ExporterKind` 静态分发透传；`ExporterManager` 提供同签名的外部接口
- **D-02:** `ExporterManager::write_template_stats()` 是 `run.rs` 的唯一调用点；两条路径（顺序/并行）调用位置均为 `exporter_manager.finalize()` 之后
- **D-03:** `final_path: Option<&Path>` 参数解决并行路径的最终 CSV 路径问题：顺序路径传 `None`（`CsvExporter` 用 `self.path`），并行路径传 `Some(Path::new(&csv_cfg.file))` （`concat_csv_parts()` 完成后的最终输出路径）

### Error 处理

- **D-04:** `write_template_stats()` 失败 → 返回 `Err`，`run.rs` 向上传播，整体退出码非零；不降级为 warn-and-continue

### Dry-run 行为

- **D-05:** `DryRunExporter::write_template_stats()` 为 no-op，只打 `info!` 摘要（与 `export_one_preparsed` / `finalize` 的 dry-run 行为一致）；不产生任何文件

### SQLite 连接重用与 DDL 策略

- **D-06:** `SqliteExporter::finalize()` 只执行 `COMMIT`，不关闭连接（`conn: Option<Connection>` 保留）；`write_template_stats()` 在同一连接上开新事务写 `sql_templates`；`SqliteExporter` drop 时连接自动关闭
- **D-07:** `sql_templates` 表行为跟随主表 overwrite/append 语义：`overwrite=true` 时 DROP IF EXISTS + CREATE TABLE；`append=true` 时 CREATE TABLE IF NOT EXISTS（保留历史行，INSERT INTO 新行）
- **D-08:** 表已存在且 overwrite 时，必须在 `write_template_stats()` 内 DROP 并重建，而非在 `initialize()` 阶段提前处理（因为 finalize 之前 stats 不可用）

### CSV 伴随文件策略

- **D-09:** `CsvExporter::write_template_stats()` 写入 `<basename>_templates.csv`：若 `final_path` 为 `Some(p)` 则用该路径推导，若为 `None` 则从 `self.path` 推导（stem + `_templates.csv`）
- **D-10:** 伴随文件始终覆盖写入（模板统计是全量结果，无追加语义），不跟随主 CSV append 标志

### 数据完整性保证

- **D-11:** `write_template_stats()` 调用在 `exporter_manager.finalize()` 之后，任何主导出提前终止（中断/错误）时 `template_agg` 会是 `None`，`write_template_stats()` 不会被调用——数据完整性由现有结构天然保证

### Claude 自行决定

- `sql_templates` 表的列类型：`template_key TEXT NOT NULL PRIMARY KEY`，数值列 `INTEGER NOT NULL`，时间戳列 `TEXT NOT NULL`
- SQL 注入防护：`sql_templates` 为固定表名，不需要 ASCII 白名单（与 `table_name` 不同）；DDL 使用固定字面量
- 批量写入：统计条目数量远小于主记录，单事务批量 INSERT 即可，无需分批

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 13 输出（直接依赖）
- `src/features/template_aggregator.rs` — `TemplateStats` struct（字段含义、单位、已 derive Serialize）；`TemplateAggregator::finalize()` 返回 `Vec<TemplateStats>`
- `.planning/phases/13-templateaggregator/13-CONTEXT.md` — D-01~D-12（耗时单位 µs、字段命名、hdrhistogram 配置）

### 直接修改的文件
- `src/exporter/mod.rs` — `Exporter` trait（新增 `write_template_stats`）；`ExporterKind`（新增透传 impl）；`ExporterManager`（新增外部方法）
- `src/exporter/sqlite.rs` — `SqliteExporter`：`finalize()` 改为保留 conn；新增 `write_template_stats()` impl
- `src/exporter/csv.rs` — `CsvExporter`：新增 `write_template_stats()` impl（写伴随文件）
- `src/cli/run.rs` — 两条路径（顺序 ~L886、并行 ~L792）在 `finalize()` 之后新增 `exporter_manager.write_template_stats(&stats, final_path)?`

### 需求与架构参考
- `.planning/ROADMAP.md` §"Phase 14" — 3 条成功标准（表结构、伴随文件结构、写入时序）
- `.planning/REQUIREMENTS.md` §"TMPL-04" — 功能需求原文
- `.planning/STATE.md` §"Decisions (v1.3)" — 锁定决策表

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `rusqlite::Connection`：`SqliteExporter` 的 `conn: Option<Connection>` 在 `finalize()` 后仍可用；`initialize_pragmas()` 已设高性能参数，无需重新设置
- `SqliteExporter::build_create_sql()` / `build_insert_sql()` 模式：参考构建 `sql_templates` 表的 DDL（但 sql_templates 字段固定，直接用字面量即可）
- CSV `ensure_parent_dir()` in `src/exporter/mod.rs`：写伴随文件前调用以确保目录存在

### Established Patterns
- `Exporter` trait 三段式生命周期（initialize → export_one_preparsed × N → finalize）：`write_template_stats()` 作为第四段，在 finalize 之后调用
- `ExporterKind` enum 静态分发：`Csv(CsvExporter) | Sqlite(SqliteExporter) | DryRun(DryRunExporter)`；新 trait 方法需在所有 variant 添加 impl
- Phase 7 SQL 注入防护：`table_name` 用 ASCII 白名单 + DDL 双引号转义；`sql_templates` 为固定名称，不需要此防护

### Integration Points
- `run.rs:886`（顺序路径）：`exporter_manager.finalize()?` 之后、`template_agg.map(TemplateAggregator::finalize)` 之后，插入 `exporter_manager.write_template_stats(&stats, None)?`
- `run.rs:792`（并行路径）：`template_stats` 计算后，插入 `exporter_manager.write_template_stats(&stats, Some(Path::new(&csv_cfg.file)))?`（注意：并行路径的 `exporter_manager` 需从函数返回或重构）
- 并行路径细节：`process_csv_parallel()` 返回后主 `ExporterManager` 是什么需要确认（并行路径不创建主 ExporterManager，只有各 rayon task 的临时 `ExporterManager`）——研究阶段需确认并行路径的 `write_template_stats` 调用者

</code_context>

<specifics>
## Specific Ideas

- `write_template_stats()` 签名中 `final_path: Option<&Path>` 参数：顺序路径传 `None`，并行路径传 `Some(&csv_output_path)`
- `sql_templates` 建表列顺序与 ROADMAP 一致：`template_key, count, avg_us, min_us, max_us, p50_us, p95_us, p99_us, first_seen, last_seen`
- 伴随文件首行写 CSV 表头（与 SQLite 表列名一致），`itoa` 写数值字段（与主 CSV 写法一致）

</specifics>

<deferred>
## Deferred Ideas

- 独立 JSON 报告输出（TMPL-03）— Future Requirements v1.4+
- 独立 CSV 报告输出（TMPL-03b）— Future Requirements v1.4+
- 图表生成（CHART-01~05）— Phase 15/16

</deferred>

---

*Phase: 14-Exporter 集成输出*
*Context gathered: 2026-05-16*

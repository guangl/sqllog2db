# Phase 14: Exporter 集成输出 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 14-Exporter 集成输出
**Areas discussed:** 写入集成点设计, Error 处理严格性, Dry-run 行为, SQLite 连接重用策略

---

## 写入集成点设计

| Option | Description | Selected |
|--------|-------------|----------|
| ExporterManager 新方法 | 给 ExporterManager 加 write_template_stats() 方法，内部按导出器类型路由 | ✓ |
| run.rs 直接操作 | 在 run.rs 里从 final_cfg 取路径，调用独立辅助函数，不改 ExporterManager 接口 | |

**User's choice:** ExporterManager 新方法

| Option | Description | Selected |
|--------|-------------|----------|
| ExporterManager 直接处理 | mod.rs 里 match 导出器类型，内联调用 rusqlite/csv 写入 | |
| 委派给各导出器 | Exporter trait 新增方法，SqliteExporter/CsvExporter/DryRunExporter 各自实现 | ✓ |

**User's choice:** 委派给各导出器（与现有 initialize/export_one/finalize 黑盒设计一致）

| Option | Description | Selected |
|--------|-------------|----------|
| 单独传入最终路径 | write_template_stats() 接收 final_path: Option<&Path> 参数 | ✓ |
| 并行路径单独处理 | 并行路径不经 ExporterManager，run.rs 直接调用独立函数 | |

**User's choice:** 单独传入最终路径（顺序路径传 None，并行路径传 Some(&csv_cfg.file)）

---

## Error 处理严格性

| Option | Description | Selected |
|--------|-------------|----------|
| Fatal（返回 Err） | 整体返回错误，退出码非零，用户能感知模板文件缺失 | ✓ |
| Warn-and-continue | 主导出成功即正常退出，模板写入失败只记录警告 | |

**User's choice:** Fatal

---

## Dry-run 行为

| Option | Description | Selected |
|--------|-------------|----------|
| 跳过（不写） | dry-run 语义为"不产生任何输出文件"，模板统计同样不写 | ✓ |
| 正常写出 | 模板分析结果可独立于主导出，--dry-run 时也写出用于验证 | |

**User's choice:** 跳过（不写）

---

## SQLite 连接重用策略

| Option | Description | Selected |
|--------|-------------|----------|
| 复用 finalize() 后的连接 | finalize() 只 COMMIT 不关闭，write_template_stats() 在同一连接开新事务 | ✓ |
| 重新打开数据库 | write_template_stats() 用 database_url 重新 Connection::open() | |

**User's choice:** 复用 finalize() 后的连接（更高效，避免重建连接开销）

| Option | Description | Selected |
|--------|-------------|----------|
| DROP + 重建 | 跟随主表 overwrite/append 语义（overwrite=DROP+CREATE，append=IF NOT EXISTS） | ✓ |
| CREATE TABLE IF NOT EXISTS + DELETE | 每次写入前 DELETE FROM，始终只保留当次运行结果 | |

**User's choice:** DROP + 重建，跟随主表语义

| Option | Description | Selected |
|--------|-------------|----------|
| 约定跟随主 CSV 设置 | overwrite 时覆盖伴随文件，append 时也覆盖（统计无追加语义） | ✓ |
| 始终覆盖 | 不考虑 append 标志，始终覆盖写入 | |

**User's choice:** 约定跟随主 CSV 设置（实际效果为始终覆盖，因统计数据无追加语义）

---

## Claude's Discretion

- `sql_templates` 表列类型（`template_key TEXT NOT NULL PRIMARY KEY`，数值列 `INTEGER NOT NULL`）
- 批量写入策略（单事务批量 INSERT，无需分批）
- SQL 注入防护方案（固定表名无需防护，区别于可配置的 `table_name`）

## Deferred Ideas

- 独立 JSON/CSV 报告输出（TMPL-03/03b）— 已移入 Future Requirements v1.4+
- 图表生成（CHART-01~05）— Phase 15/16

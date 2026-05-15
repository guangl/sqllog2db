# Phase 7: 技术债修复 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-10
**Phase:** 7-技术债修复
**Areas discussed:** DEBT-01 错误严重性, 错误输出目标, table_name 校验规则

---

## DEBT-01 错误严重性

| Option | Description | Selected |
|--------|-------------|----------|
| warn + 继续 | log::warn!() 记录错误，initialize() 照常继续 | ✓ |
| 硬失败 — 中止运行 | 返回 Err()，initialize() 失败导致整个 run 中止 | |
| 静默忽略非致命错误 | 任何 DELETE 错误均静默，仅 debug 日志记录 | |

**User's choice:** warn + 继续
**Notes:** DELETE FROM 失败不影响后续 CREATE TABLE + INSERT 流程，不应中止运行。

---

## 错误输出目标

| Option | Description | Selected |
|--------|-------------|----------|
| 应用日志 log::warn!() | 写入 [logging] file（logs/sqllog2db.log），不改动 exporter 结构 | ✓ |
| 解析错误文件 [error] file | 写入 export/errors.log，需修改 SqliteExporter 结构传入 error writer | |

**User's choice:** 应用日志 log::warn!()
**Notes:** 最小侵入性方案。[error] file 在语义上属于解析错误，不适合混入 exporter 初始化错误。

---

## table_name 校验规则

**问题 1：合法字符如何定义？**

| Option | Description | Selected |
|--------|-------------|----------|
| ASCII 标识符 | ^[a-zA-Z_][a-zA-Z0-9_]*$，起头字母或下划线 | ✓ |
| 禁止 SQL 特殊字符 | 允许 Unicode 字母，禁止 ; " ' - % \ 等 | |

**User's choice:** ASCII 标识符
**Notes:** 项目场景（达梦 SQL 日志）无需 Unicode 表名，严格规则更安全。

**问题 2：双引号转义覆盖范围？**

| Option | Description | Selected |
|--------|-------------|----------|
| 全部 4 条 DDL | DROP / DELETE / CREATE / INSERT 全部转义 | ✓ |
| 仅 format! 直接拼接处 | 只改 DROP/DELETE，忽略 build_create_sql / build_insert_sql | |

**User's choice:** 全部 4 条 DDL
**Notes:** 与 success criteria 一致，防御深度优先。

---

## Claude's Discretion

- "no such table" 判断具体实现方式（error message 子串匹配 vs extended_code 常量）——已在 CONTEXT.md specifics 中记录倾向，planner 可根据 rusqlite API 最终确定

## Deferred Ideas

None

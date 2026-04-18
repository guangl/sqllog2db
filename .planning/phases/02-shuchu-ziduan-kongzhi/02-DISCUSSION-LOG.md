# Phase 2: 输出字段控制 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.

**Date:** 2026-04-18
**Phase:** 02-shuchu-ziduan-kongzhi
**Areas discussed:** 列顺序语义, 空列表处理, normalized_sql 联动

---

## 列顺序语义

| Option | Description | Selected |
|--------|-------------|----------|
| A. 按配置顺序 | 用 Vec<usize> 有序索引，ROADMAP"列顺序与配置一致" | ✓ |
| B. 按固定原始顺序 | 直接用现有 FieldMask bitmask，最简单 | |

**User's choice:** A — 按配置顺序
**Notes:** 需在 FieldMask 之外额外存储 Vec<usize> 有序索引

---

## 空列表处理

| Option | Description | Selected |
|--------|-------------|----------|
| A. 等同于"导出全部" | 与不配置效果一致，零歧义 | ✓ |
| B. 启动报错 | 防止用户误配置 | |

**User's choice:** A — 等同于导出全部，不报错

---

## normalized_sql 联动

| Option | Description | Selected |
|--------|-------------|----------|
| A. 静默忽略 | fields 说什么就导什么，replace_parameters 结果丢弃 | ✓ |
| B. 给出警告 | 防止用户困惑 | |

**User's choice:** A — 静默忽略，无警告

---

## Deferred Ideas

无

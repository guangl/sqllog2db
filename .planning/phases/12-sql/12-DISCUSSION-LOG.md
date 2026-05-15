# Phase 12: SQL 模板归一化引擎 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-15
**Phase:** 12-SQL 模板归一化引擎
**Areas discussed:** IN 列表折叠粒度, 关键字大小写方向, 与 fingerprint 的代码关系, TemplateAnalysisConfig 预定义范围

---

## IN 列表折叠粒度

| Option | Description | Selected |
|--------|-------------|----------|
| IN (?) — 单一占位符 | 任意长度 IN 列表折叠为 IN (?)，最大化语义相同 SQL 的合并率 | ✓ |
| IN (?,?,?) — 保留占位符数量 | 列表长度不同得到不同 key，区分单查询与批量查询 | |
| IN (...) — 不替换 | 只删注释和空白，不替换 IN 列表内容 | |

**User's choice:** IN (?) — 单一占位符，覆盖所有字面量（数字 + 字符串）

---

**覆盖范围追问：**

| Option | Description | Selected |
|--------|-------------|----------|
| 包括所有字面量清单 | IN (1,2,3) 和 IN ('a','b','c') 都 → IN (?) | ✓ |
| 仅数字列表 | 字符串列表 IN ('a','b') 保留原样 | |

**User's choice:** 包括所有字面量清单

---

## 关键字大小写方向

| Option | Description | Selected |
|--------|-------------|----------|
| 全部大写 | SELECT/FROM/WHERE/AND 等，符合 SQL 传统 | ✓ |
| 全部小写 | select/from/where，部分工具默认小写 | |

**User's choice:** 全部大写

---

## 与 fingerprint 的代码关系

**模块放置：**

| Option | Description | Selected |
|--------|-------------|----------|
| 新建 template_analysis.rs | 独立模块，边界清晰，不影响 fingerprint | |
| 扩展 sql_fingerprint.rs | 共享字节扫描引擎 | ✓ |

**User's choice:** 扩展 sql_fingerprint.rs

---

**代码共享方式：**

| Option | Description | Selected |
|--------|-------------|----------|
| 共享底层扫描循环结构 | 抽取 scan_sql_bytes()，NEEDS_SPECIAL 表只写一次 | ✓ |
| 各自独立实现 | normalize_template() 有独立循环，存在重复代码 | |

**User's choice:** 共享底层扫描循环结构

---

## TemplateAnalysisConfig 预定义范围

| Option | Description | Selected |
|--------|-------------|----------|
| 仅 enabled: bool | 最小化，后续阶段按需拉伸 | ✓ |
| 预定义 enabled + top_n: usize | 提前锁定接口，避免后续 TOML 改动 | |

**User's choice:** 仅 enabled: bool

---

## Claude's Discretion

- 共享扫描引擎的具体抽象形式（泛型函数 vs. enum 策略 vs. 其他）——由 planner/executor 决定，约束是不破坏 fingerprint() 现有行为
- 关键字识别列表的完整枚举——标准 SQL DML/DDL 关键字即可，无需穷举达梦方言

## Deferred Ideas

- `TemplateAnalysisConfig` 后续字段（`top_n` 等）— 推迟到 Phase 14/15
- normalize_template() 的 key 截断功能 — 超出本阶段范围

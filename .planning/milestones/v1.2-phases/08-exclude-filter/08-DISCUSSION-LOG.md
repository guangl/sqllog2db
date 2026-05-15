# Phase 8: 排除过滤器 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-10
**Phase:** 8-排除过滤器
**Areas discussed:** TOML 配置结构, init 模板更新, 快路径激活条件

---

## TOML 配置结构

### Q1: exclude 规则放在哪里？

| Option | Description | Selected |
|--------|-------------|----------|
| 平铺加前缀 | `exclude_usernames` 等键平铺在 [features.filters] 下，serde flatten 直接支持 | ✓ |
| [features.filters.exclude] 子段 | 独立子段，字段名与 include 相同，分组清晰但多一层嵌套 | |

**User's choice:** 平铺加前缀（exclude_usernames, exclude_client_ips 等）

### Q2: 键名复数还是单数？

| Option | Description | Selected |
|--------|-------------|----------|
| 复数（推荐） | exclude_usernames / exclude_client_ips，与现有 include 字段完全对称 | ✓ |
| 单数 | exclude_username / exclude_client_ip，与内部字段命名不对称 | |

**User's choice:** 复数，与现有 include 字段对称

---

## init 模板更新

### Q1: 是否加入注释示例？

| Option | Description | Selected |
|--------|-------------|----------|
| 加入注释示例 | 在 meta filters 区域加入注释掉的 exclude 示例，用户可发现并参考 | ✓ |
| 不加 | 保持简洁，用户查文档即可 | |

**User's choice:** 加入注释示例

### Q2: exclude 示例如何组织？

| Option | Description | Selected |
|--------|-------------|----------|
| 每个字段紧掘 include 下方 | usernames 示例正下方放 exclude_usernames，配对感最强 | ✓ |
| 所有 exclude 集中放区域尾部 | include 字段不动，统一放 --- Exclude filters --- 分隔后 | |

**User's choice:** 每个字段紧挨 include 下方

---

## 快路径激活条件

### Q1: has_filters() 如何扩展（纯 exclude 配置时）？

| Option | Description | Selected |
|--------|-------------|----------|
| has_filters() 检查 exclude 字段 | 扩展 MetaFilters::has_filters() 同时检查 include + exclude，任一非空即激活 pipeline | ✓ |
| 要求 enable=true 才检查 | 现有逻辑已有 enable flag，无需改变语义 | |

**User's choice:** 扩展 has_filters() 检查 exclude 字段

### Q2: has_meta_filters 预计算是否包含 exclude？

| Option | Description | Selected |
|--------|-------------|----------|
| has_meta_filters 包含 exclude | has_meta_filters = include 或 exclude 任一配置即 true | ✓ |
| 分开预计算 | has_meta_includes / has_meta_excludes 分别预计算 | |

**User's choice:** has_meta_filters 包含 exclude（统一预计算）

### Q3: 编译后的 exclude 正则放在哪里？

| Option | Description | Selected |
|--------|-------------|----------|
| 展展 CompiledMetaFilters 结构体 | 直接加 exclude_* 字段，should_keep() 先 exclude 后 include | ✓ |
| 新建 CompiledExcludeFilters 结构体 | FilterProcessor 持有两个结构体，分离但逻辑分散 | |

**User's choice:** 扩展 CompiledMetaFilters，should_keep() 内先 exclude 短路再 include

---

## Claude's Discretion

无——所有关键决策均由用户明确选择。

## Deferred Ideas

无——讨论完全在 Phase 8 范围内进行。

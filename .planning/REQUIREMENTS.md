# REQUIREMENTS.md — Milestone v1.0
# 增强 SQL 内容过滤与字段投影

**Milestone:** v1.0  
**Status:** Active  
**Created:** 2026-04-18

---

## v1.0 Requirements

### 过滤器增强 (Filter)

- [ ] **FILTER-01**: 用户可对任意字段（sql_text/user/schema/ip 等）配置正则表达式过滤条件，运行时仅保留所有正则均匹配的记录
- [ ] **FILTER-02**: 多个过滤条件列表默认 AND 语义——全部条件满足才保留该记录

### 字段控制 (Field)

- [ ] **FIELD-01**: 用户可在 config.toml 中指定导出哪些字段（列名列表），未指定则导出全部字段

---

## Future Requirements

- **FILTER-03**: 支持排除模式——过滤条件匹配则丢弃记录（而非匹配则保留）

---

## Out of Scope

- **OR 条件组合** — 简单列表 AND 语义已满足主要需求，OR 增加配置复杂度
- **跨字段联合条件** — 暂不支持"字段A 满足 X 且 字段B 满足 Y"的复合谓词
- **运行时热重载** — 配置在启动时加载，不支持动态修改过滤规则

---

## Traceability

| REQ-ID | Phase | Plan |
|--------|-------|------|
| FILTER-01 | — | — |
| FILTER-02 | — | — |
| FIELD-01 | — | — |

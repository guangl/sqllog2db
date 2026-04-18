# Roadmap: sqllog2db v1.0 增强 SQL 内容过滤与字段投影

## Overview

本里程碑为用户提供两项精确控制能力：一是对任意字段配置正则过滤（AND 语义），二是在配置文件中指定导出哪些字段。两个阶段交付后，用户可以完整控制"导出哪些记录的哪些字段"。

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

- [x] **Phase 1: 正则字段过滤** - 支持对任意字段配置正则表达式条件，多条件 AND 语义 (Completed: 2026-04-18)
- [ ] **Phase 2: 输出字段控制** - 用户可在 config.toml 中指定导出哪些字段

## Phase Details

### Phase 1: 正则字段过滤
**Goal**: 用户可以通过配置对任意字段设置正则过滤，并且多条件之间自动应用 AND 语义
**Depends on**: Nothing (first phase)
**Requirements**: FILTER-01, FILTER-02
**Success Criteria** (what must be TRUE):
  1. 用户在 config.toml 中对 sql_text/user/schema/ip 等任意字段配置正则后，运行时只有所有正则均匹配的记录被导出
  2. 配置多个过滤条件时，只有全部条件同时满足的记录才被保留（AND 语义）
  3. 未配置任何过滤条件时，行为与之前完全一致（无性能损耗，pipeline.is_empty() 快路径生效）
  4. 正则表达式格式错误时，工具在启动阶段报错并给出明确提示，而非在运行时静默失败
**Plans:** 2 plans

Plans:
- [x] 01-01-PLAN.md — 正则核心实现：regex 依赖 + CompiledMetaFilters/CompiledSqlFilters + 验证 + 测试
- [x] 01-02-PLAN.md — 热路径集成：FilterProcessor 使用编译后的正则结构

### Phase 2: 输出字段控制
**Goal**: 用户可以在 config.toml 中指定一个字段列表，导出结果只包含列出的字段；未指定则导出全部字段
**Depends on**: Phase 1
**Requirements**: FIELD-01
**Success Criteria** (what must be TRUE):
  1. 用户在 config.toml 中配置 fields 列表后，CSV/SQLite 输出只包含指定的字段列（列顺序与配置一致）
  2. config.toml 中不配置 fields 时，输出包含所有字段，与现有行为完全相同
  3. fields 中指定了不存在的字段名时，工具在启动阶段报错提示无效字段名，不会静默忽略
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. 正则字段过滤 | 2/2 | Complete | 2026-04-18 |
| 2. 输出字段控制 | 0/? | Not started | - |

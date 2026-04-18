# Phase 1: 正则字段过滤 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-18
**Phase:** 01-正则字段过滤
**Areas discussed:** Config 格式, 同字段多值语义, SQL 字段正则层级, 跨字段 AND 范围

---

## Config 格式

| Option | Description | Selected |
|--------|-------------|----------|
| 升级现有字段 | usernames/client_ips 等字段直接接受正则语法，配置格式不变，向后兼容 | ✓ |
| 新增 regex_ 前缀字段 | 现有字段保持子串匹配，新增 regex_usernames 等专属字段 | |

**User's choice:** 升级现有字段
**Notes:** 纯字符串本身就是合法正则，向后兼容现有配置

---

## 同字段多值语义

| Option | Description | Selected |
|--------|-------------|----------|
| OR（任意匹配即保留） | 列表内多个正则是"允许名单"语义，任一匹配即满足该字段条件 | ✓ |
| AND（全部匹配才保留） | 列表内所有正则都必须匹配，对同一字段实用性较低 | |

**User's choice:** OR（任意匹配）
**Notes:** 跨字段依然是 AND，同字段内 OR 符合直觉

---

## SQL 字段正则层级

| Option | Description | Selected |
|--------|-------------|----------|
| 只记录级 | record_sql 升级为正则，主循环中 DML 记录独立判断，无预扫描开销 | ✓ |
| 两层都支持 | record_sql + sql（事务级）都支持正则，事务级需预扫描 | |

**User's choice:** 只记录级
**Notes:** 简化实现，避免预扫描层的额外复杂度

---

## 跨字段 AND 范围

| Option | Description | Selected |
|--------|-------------|----------|
| 所有字段都 AND | username + client_ip + appname 等所有配置字段同时满足才保留 | ✓ |
| 时间 AND，其他字段 OR | 保持现有跨字段 OR 逻辑，只有时间范围是 AND | |

**User's choice:** 所有字段都 AND
**Notes:** 符合 FILTER-02 "全部条件满足才保留"的要求

---

## Claude's Discretion

- 正则编译时机（启动时 vs 运行时）——启动时编译，启动阶段报错
- 使用 `regex::Regex` 还是 `regex::RegexSet` 实现多值 OR 匹配
- 具体的 `has_meta_filters` 预计算逻辑更新方式

## Deferred Ideas

- 事务级 sql 过滤的正则升级 — 预扫描层，留待后续

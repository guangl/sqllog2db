# Requirements: sqllog2db v1.2

**Defined:** 2026-05-10
**Core Value:** 用户能够精确指定"导出哪些记录的哪些字段"——过滤逻辑清晰可配置，输出结果完全可控。

## v1.2 Requirements

### Tech Debt

- [ ] **DEBT-01**: 用户在 SQLite 导出时，初始化阶段的 DELETE 错误按类型区分——表不存在等无害错误忽略，其他错误写入 error log
- [ ] **DEBT-02**: 用户配置的 `table_name` 在启动时校验（白名单字符），且所有 DDL 语句（DROP/DELETE/CREATE/INSERT）使用双引号转义表名防止 SQL 注入
- [ ] **DEBT-03**: Phase 3/4/5/6 的 VALIDATION.md Nyquist compliant 状态补签完整

### 过滤功能

- [ ] **FILTER-03**: 用户可在 config 中指定排除模式——任意元数据字段匹配 exclude 规则的记录被丢弃（OR veto 语义）；支持所有 7 个元数据字段；空配置不引入额外开销（保留 `pipeline.is_empty()` 快路径）

### 性能

- [ ] **PERF-10**: 在 FILTER-03 就位后重新 profile 热路径（criterion + flamegraph）；若发现 >5% 可消除热点则实施相应优化；否则记录"已达当前瓶颈"并签署报告
- [ ] **PERF-11**: 用 hyperfine 量化 CLI 冷启动基线；消除双重 regex 编译（validate 与 compile 阶段合并为 `validate_and_compile()`）；若 update check 在启动占比显著（>50ms）则后台线程化

## Future Requirements

### 过滤功能

- **FILTER-04**: OR 条件组合——当前 AND 语义已满足主要需求，OR 增加配置复杂度
- **FILTER-05**: 跨字段联合条件——"字段A 满足 X 且 字段B 满足 Y"的复合谓词

## Out of Scope

| Feature | Reason |
|---------|--------|
| 运行时热重载过滤规则 | 配置在启动时加载，流式设计不支持中途修改 |
| `exclude_trxids` 正则支持 | 保持与 include_trxids 的 HashSet 精确匹配对称性 |
| SQLite WAL 模式 | 用户决策移除：数据无需崩溃保护（v1.1） |
| JSON / Parquet 导出 | 超出当前里程碑范围 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DEBT-01 | Phase 7 | Pending |
| DEBT-02 | Phase 7 | Pending |
| DEBT-03 | Phase 11 | Pending |
| FILTER-03 | Phase 8 | Pending |
| PERF-10 | Phase 10 | Pending |
| PERF-11 | Phase 9 | Pending |

**Coverage:**
- v1.2 requirements: 6 total
- Mapped to phases: 6
- Unmapped: 0 ✓

---
*Requirements defined: 2026-05-10*
*Last updated: 2026-05-10 after initial definition*

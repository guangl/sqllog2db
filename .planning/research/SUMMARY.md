# Research Summary — sqllog2db v1.2

**Milestone:** v1.2 质量强化 & 性能深化
**Confidence:** HIGH — 所有结论直接来自源码检查

---

## Executive Summary

v1.2 是在已有健壮基础上执行精确外科手术的版本。所有新特性均可在**零新依赖**的情况下完成：排除过滤器（FILTER-03）通过镜像现有 `CompiledMetaFilters` include 字段结构实现；性能优化（PERF-10/11）利用已安装的 criterion + hyperfine + flamegraph 基础设施；技术债修复（DEBT-01/02）是局部代码变更。`aho-corasick`、`regex-lite`、`lazy_static` 均经评估后明确排除。

---

## Stack Additions

**零新 crate** — `Cargo.toml` 保持不变。

| 工具 | 用途 | 状态 |
|------|------|------|
| `regex 1.12.3` | 预编译 exclude 正则，内部 memchr SIMD | 已有 |
| `criterion 0.7.0` | PERF-10 热路径 benchmark | 已有 |
| `hyperfine 1.20.0` | PERF-11 冷启动测量 | 已安装 |
| table_name 白名单 | `str::chars().all(alphanumeric\|_)` + 双引号转义 | 纯代码 |

**排除方案：**
- `aho-corasick`：<10 条 pattern 规模无优势，regex 已 SIMD 加速
- `regex-lite`：降低 Unicode 支持，静默语义差异风险不可接受
- `lazy_static`：OnceLock 与 Config::Clone + serde 不兼容

---

## Feature Landscape

### P1 必须交付

| Feature | 实现方案 | 复杂度 |
|---------|---------|--------|
| FILTER-03 排除模式 | MetaFilters + CompiledMetaFilters 增加 7 个 `exclude_*` 字段，`should_keep()` 追加 OR veto 逻辑 | 低 |
| DEBT-01 SQLite 静默错误 | `let _ =` → 按 error code 区分：无害忽略，其他 warn 到 error log | 极低 |
| DEBT-02 SQL 注入 | 4 处 DDL 拼接加白名单校验 + 双引号转义 | 极低 |
| DEBT-03 Nyquist 补签 | Phase 3/4/5/6 VALIDATION.md 文档补全 | 文档 |

### P2 条件交付（门控）

| Feature | 门控条件 | 工具 |
|---------|---------|------|
| PERF-11 启动提速 | hyperfine 实测 validate 冷启动 >50ms | hyperfine |
| PERF-10 热路径优化 | flamegraph 显示 >5% 可消除热点 | criterion + flamegraph |

### 明确排除

- 跨字段 OR 排除组合
- 运行时热重载
- `exclude_trxids` 正则支持（保持 HashSet 精确匹配对称性）

---

## Architecture

### FILTER-03 集成方案

**选择 Option A**：在 `CompiledMetaFilters::should_keep()` 内追加排除逻辑（而非独立 ExcludeProcessor），避免 `process_with_meta` 双调用开销。

排除顺序：**先排除后包含**（exclusion 是 hard veto，pattern 少时短路更快）。

```
MetaFilters 新增字段:
  exclude_usernames: Vec<String>
  exclude_client_ips: Vec<String>
  exclude_sess_ids: Vec<String>
  exclude_thrd_ids: Vec<String>
  exclude_statements: Vec<String>
  exclude_appnames: Vec<String>
  exclude_tags: Vec<String>

CompiledMetaFilters 新增字段:
  compiled_exclude_usernames: Vec<Regex>
  ... (对应 7 个字段)

should_keep() 逻辑:
  1. exclude veto (OR 语义): 任意 exclude 字段命中 → return false
  2. include check (AND 语义): 与现有逻辑不变
```

### PERF-11 启动优化

`Config::validate_and_compile()` 消除双重 regex 编译：`validate_regexes()` 仅验证语法，`from_meta()` / `from_sql_filters()` 再编译一次——合并为一次。

`check_for_updates_at_startup()` 移入后台线程，超时 200ms 放弃 join。

### DEBT-01

`sqlite.rs:initialize()` 中 `let _ = conn.execute("DELETE FROM ...")` — 按 `rusqlite::ErrorCode` 细分：`SqliteFailure { code: Unknown .. }` 忽略，其他写 error log。

### DEBT-02

四处 DDL 拼接均加固：DROP / DELETE / CREATE / INSERT 的 `table_name` 全部加双引号转义，且在 `Config::validate()` 阶段拦截非法字符。

---

## Top Pitfalls

| # | 陷阱 | 预防 | Phase |
|---|------|------|-------|
| 1 | 破坏 `pipeline.is_empty()` 快路径 | `has_exclude_filters()` 守卫，空配置不加 processor | Phase 2 |
| 2 | 排除语义 AND 错误（应为 OR veto） | 独立 `any()` 实现，参考 `CompiledSqlFilters::matches()` | Phase 2 |
| 3 | `validate_regexes()` 漏掉新 exclude_* 字段 → 运行时 panic | validate 阶段覆盖所有新 regex 字段 | Phase 2 |
| 4 | 热路径动态编译 exclude regex | `FilterProcessor::new()` 预编译，不得在 `process_with_meta()` 里 `Regex::new()` | Phase 2 |
| 5 | DEBT-02 修复不彻底（只改 1 处） | 四处 DDL 全部加双引号 | Phase 1 |

---

## Recommended Phase Order

| Phase | 内容 | 依赖 | Files |
|-------|------|------|-------|
| 1 | DEBT-01 + DEBT-02 | 无 | sqlite.rs, config.rs |
| 2 | FILTER-03 | 无 | filters.rs, init.rs |
| 3 | PERF-11（门控） | FILTER-03 完成 | config.rs, run.rs, main.rs |
| 4 | PERF-10（门控） | FILTER-03 + PERF-11 | filters.rs, criterion/ |
| 5 | DEBT-03 Nyquist | 无 | VALIDATION.md ×4 |

Phase 1/2/5 直接排期；Phase 3/4 以实测数据作为门控条件。

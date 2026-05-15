# Phase 12: SQL 模板归一化引擎 - Context

**Gathered:** 2026-05-15
**Status:** Ready for planning

<domain>
## Phase Boundary

实现 `normalize_template(sql: &str) -> String` 函数，对 sql_text 执行四项变换（注释去除、IN 列表折叠、关键字大小写统一、多余空白规范化），输出稳定的模板 key。同时引入 `TemplateAnalysisConfig` 配置结构并接入 `FeaturesConfig`，让用户可在 config 中通过 `[features.template_analysis] enabled = true` 启用归一化。

本阶段 **不涉及** 统计聚合（Phase 13）、结果输出（Phase 14）或图表生成（Phase 15）。

</domain>

<decisions>
## Implementation Decisions

### IN 列表折叠

- **D-01:** `IN (1, 2, 3)` → `IN (?)`，折叠为单一占位符
- **D-02:** 覆盖所有字面量类型：数字列表 **和** 字符串列表均折叠，`IN ('a', 'b', 'c')` → `IN (?)`
- **D-03:** 字面量数量不同的 IN 列表（如 `IN (1,2)` vs `IN (1,2,3,4,5)`）产生**相同**的模板 key

### 关键字大小写

- **D-04:** 关键字统一为**全部大写**（`SELECT`、`FROM`、`WHERE`、`AND`、`OR`、`JOIN`、`ON`、`AS`、`GROUP BY`、`ORDER BY`、`HAVING` 等 SQL 保留字）
- **D-05:** 非关键字标识符（表名、列名、别名等）保留原始大小写

### 代码放置与复用

- **D-06:** `normalize_template()` 放入现有 `src/features/sql_fingerprint.rs`，与 `fingerprint()` 并列
- **D-07:** 抽取私有辅助函数 `scan_sql_bytes()`（或等价结构），供 `fingerprint()` 和 `normalize_template()` 共享底层字节扫描循环
- **D-08:** `NEEDS_SPECIAL` 字节表、`memchr` SIMD 查找等底层逻辑只写一次，两个函数通过不同的处理策略参数化
- **D-09:** 对外暴露路径不变：`src/features/mod.rs` 新增 `pub use sql_fingerprint::normalize_template` 导出

### TemplateAnalysisConfig

- **D-10:** `TemplateAnalysisConfig` 仅含 `enabled: bool`，不预定义后续阶段字段
- **D-11:** 放入 `src/features/mod.rs`，嵌套在 `FeaturesConfig` 下（与 `ReplaceParametersConfig` 并列）
- **D-12:** TOML 路径：`[features.template_analysis]`，字段 `enabled = true/false`（默认 `false`）

### 归一化在热循环中的调用

- **D-13:** 调用位置与 `compute_normalized()` 类似——在 `cli/run.rs` 热循环中，仅当 `template_analysis.enabled` 为 `true` 时调用 `normalize_template()`；禁用时零开销
- **D-14:** 归一化结果（template key）暂存为局部变量，供 Phase 13 的 `TemplateAggregator::observe()` 使用（Phase 13 实现）

### 正确性约束

- **D-15:** 字符串字面量内部的 `--` 和 `/* */` 不视为注释——注释去除逻辑必须在解析到字符串引号时跳过字面量内容
- **D-16:** 单行注释（`--`）去除到行尾；多行注释（`/* ... */`）去除整个注释块，替换为单空格（避免两侧 token 粘连）

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 现有归一化 / 指纹实现（直接复用基础）
- `src/features/sql_fingerprint.rs` — 现有 `fingerprint()` 实现；`NEEDS_SPECIAL` 字节表 + memchr SIMD 扫描结构是 `normalize_template()` 的共享基础
- `src/features/replace_parameters.rs` — 参数替换实现；了解字符串字面量解析模式（引号内容跳过逻辑）

### 配置扩展参考
- `src/config.rs` — `FeaturesConfig` 结构；`ReplaceParametersConfig` 是 `TemplateAnalysisConfig` 的直接仿照对象
- `src/features/mod.rs` — 模块导出模式；新函数需在此添加 `pub use sql_fingerprint::normalize_template`

### 热循环集成参考
- `src/cli/run.rs` — `compute_normalized()` 的条件调用模式是 `normalize_template()` 调用的直接参照

### 需求定义
- `.planning/ROADMAP.md` §"Phase 12" — 四项变换定义 + 成功标准（含字符串字面量保护要求）
- `.planning/PROJECT.md` §"Active" — TMPL-01 需求条目

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `fingerprint()` in `src/features/sql_fingerprint.rs:28` — 字节级扫描引擎（`NEEDS_SPECIAL` 表 + bulk copy + memchr 字符串跳过）；`normalize_template()` 共享相同结构，添加注释去除 + 大写化 + IN 折叠分支
- `NEEDS_SPECIAL: [bool; 256]` 常量 — 可扩展以覆盖 `/`（注释起始）、`-`（单行注释起始）等新增特殊字节
- `src/features/mod.rs` 的 `pub use sql_fingerprint::fingerprint` 模式 — 新函数照此导出

### Established Patterns
- **条件调用模式**：`compute_normalized()` 在 `cli/run.rs` 中以 `if do_normalize && field_active` 守卫调用，`normalize_template()` 应采用相同的 `if template_analysis.enabled` 守卫
- **`from_config` 构造器**：所有 feature 组件都通过 `from_config(&cfg)` 构造，`TemplateAnalysisConfig` 遵循此模式
- **`CompactString` 用于短字符串**：SQL 模板 key 通常 < 24 字节（对于短查询）但可能更长；返回类型用 `String`（与 `fingerprint()` 一致）

### Integration Points
- `FeaturesConfig`（`src/config.rs`）→ 新增 `template_analysis: TemplateAnalysisConfig` 字段
- `cli/run.rs` 热循环 → 在 `compute_normalized()` 调用附近添加条件调用 `normalize_template()`
- `src/features/mod.rs` → `pub use sql_fingerprint::normalize_template`

</code_context>

<specifics>
## Specific Ideas

- 共享扫描引擎时，考虑提取私有函数 `scan_sql_bytes<F>(sql: &str, handler: F) -> String` 或用 trait/enum 参数化变换策略；具体抽象形式由 planner 决定，关键约束是不能破坏 `fingerprint()` 的现有行为和性能
- 关键字识别范围：标准 SQL DML/DDL 关键字（SELECT、FROM、WHERE、INSERT、UPDATE、DELETE、CREATE、DROP、ALTER、JOIN、ON、AS、GROUP、BY、ORDER、HAVING、UNION、DISTINCT、LIMIT 等）；不需要穷举达梦方言关键字

</specifics>

<deferred>
## Deferred Ideas

- `TemplateAnalysisConfig` 的后续字段（如 `top_n: usize`）— 推迟到 Phase 14/15 按需添加
- `normalize_template()` 的 `Option<top_n>` 截断功能 — 超出本阶段范围

</deferred>

---

*Phase: 12-SQL 模板归一化引擎*
*Context gathered: 2026-05-15*

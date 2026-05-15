# Phase 12: SQL 模板归一化引擎 - Research

**Researched:** 2026-05-15
**Domain:** Rust 字节级 SQL 文本变换 + 配置结构扩展
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**IN 列表折叠**
- D-01: `IN (1, 2, 3)` → `IN (?)`，折叠为单一占位符
- D-02: 覆盖所有字面量类型：数字列表和字符串列表均折叠，`IN ('a', 'b', 'c')` → `IN (?)`
- D-03: 字面量数量不同的 IN 列表（如 `IN (1,2)` vs `IN (1,2,3,4,5)`）产生相同的模板 key

**关键字大小写**
- D-04: 关键字统一为全部大写（SELECT、FROM、WHERE、AND、OR、JOIN、ON、AS、GROUP BY、ORDER BY、HAVING 等 SQL 保留字）
- D-05: 非关键字标识符（表名、列名、别名等）保留原始大小写

**代码放置与复用**
- D-06: `normalize_template()` 放入现有 `src/features/sql_fingerprint.rs`，与 `fingerprint()` 并列
- D-07: 抽取私有辅助函数 `scan_sql_bytes()`（或等价结构），供 `fingerprint()` 和 `normalize_template()` 共享底层字节扫描循环
- D-08: `NEEDS_SPECIAL` 字节表、memchr SIMD 查找等底层逻辑只写一次，两个函数通过不同的处理策略参数化
- D-09: 对外暴露路径不变：`src/features/mod.rs` 新增 `pub use sql_fingerprint::normalize_template` 导出

**TemplateAnalysisConfig**
- D-10: `TemplateAnalysisConfig` 仅含 `enabled: bool`，不预定义后续阶段字段
- D-11: 放入 `src/config.rs`，嵌套在 `FeaturesConfig` 下（与 `ReplaceParametersConfig` 并列）
- D-12: TOML 路径：`[features.template_analysis]`，字段 `enabled = true/false`（默认 `false`）

**归一化在热循环中的调用**
- D-13: 调用位置与 `compute_normalized()` 类似——在 `cli/run.rs` 热循环中，仅当 `template_analysis.enabled` 为 `true` 时调用 `normalize_template()`；禁用时零开销
- D-14: 归一化结果（template key）暂存为局部变量，供 Phase 13 的 `TemplateAggregator::observe()` 使用（Phase 13 实现）

**正确性约束**
- D-15: 字符串字面量内部的 `--` 和 `/* */` 不视为注释——注释去除逻辑必须在解析到字符串引号时跳过字面量内容
- D-16: 单行注释（`--`）去除到行尾；多行注释（`/* ... */`）去除整个注释块，替换为单空格（避免两侧 token 粘连）

### Claude's Discretion

- 共享扫描引擎的具体抽象形式：提取私有函数 `scan_sql_bytes<F>(sql: &str, handler: F) -> String` 或用 trait/enum 参数化变换策略，关键约束是不能破坏 `fingerprint()` 的现有行为和性能
- 关键字识别范围：标准 SQL DML/DDL 关键字（SELECT、FROM、WHERE、INSERT、UPDATE、DELETE、CREATE、DROP、ALTER、JOIN、ON、AS、GROUP、BY、ORDER、HAVING、UNION、DISTINCT、LIMIT 等）；不需要穷举达梦方言关键字

### Deferred Ideas (OUT OF SCOPE)

- `TemplateAnalysisConfig` 的后续字段（如 `top_n: usize`）— 推迟到 Phase 14/15 按需添加
- `normalize_template()` 的 `Option<top_n>` 截断功能 — 超出本阶段范围
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TMPL-01 | 用户可在 config 中启用模板归一化，`normalize_template()` 在 `replace_parameters` 之后对 sql_text 执行注释去除、IN 列表折叠、关键字大小写统一，生成稳定的模板 key | 四项变换均有现有字节级扫描基础设施可复用（sql_fingerprint.rs），TemplateAnalysisConfig 参照 ReplaceParametersConfig 模式添加 |
</phase_requirements>

---

## Summary

Phase 12 是纯 Rust 内部实现任务，**不引入任何新依赖**。项目已有的 `fingerprint()` 函数（`src/features/sql_fingerprint.rs`）提供了完整的字节级 SQL 扫描基础：`NEEDS_SPECIAL` 查找表 + `memchr` SIMD 字符串字面量跳过。`normalize_template()` 与 `fingerprint()` 的核心差异在于需要额外处理四项变换：注释去除（`--` 和 `/* */`）、IN 列表折叠（`IN (?, ?, ?)` → `IN (?)`）、SQL 关键字大写化、多余空白折叠。

现有 `fingerprint()` 的字符串字面量跳过逻辑（基于 `memchr` 内循环）可直接移植为注释去除的字面量保护基础（D-15）。`ReplaceParametersConfig` 在 `src/features/mod.rs` 中的完整模式（serde 反序列化 + `default_true` + `Default` impl + `FeaturesConfig` 嵌套）是 `TemplateAnalysisConfig` 的逐字仿照对象。热循环中 `do_normalize` 变量及其在 `cli/run.rs` 第 206 行附近的条件调用模式是 `do_template` 开关的直接参照。

**主要建议：** 将四项变换编码为带状态的枚举或闭包参数，在单遍字节扫描中顺序应用，避免对 SQL 字符串进行多次 pass。注释去除和关键字大写化是新增的复杂度；IN 折叠可作为后处理步骤在输出 buffer 上执行（扫描 `IN (` 标记再折叠内容），减少主循环状态机复杂度。

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| normalize_template() 函数实现 | `src/features/sql_fingerprint.rs` | — | D-06 锁定；与 fingerprint() 共享扫描基础设施（D-07/D-08） |
| TemplateAnalysisConfig 配置结构 | `src/features/mod.rs`（FeaturesConfig 嵌套） | `src/config.rs`（Config.features 引用） | D-11 锁定；与 ReplaceParametersConfig 并列 |
| 热循环条件调用 | `src/cli/run.rs` process_log_file() | — | D-13 锁定；参照 do_normalize 守卫模式 |
| 公开导出 | `src/features/mod.rs` | — | D-09 锁定；pub use 模式统一 |

---

## Standard Stack

### Core（无新依赖）

| 库 | 已有版本 | 用途 | 备注 |
|----|---------|------|------|
| `memchr` | 2.x（已在 Cargo.toml） | 字符串字面量 SIMD 跳过；注释内 `*/` 搜索 | [VERIFIED: Cargo.toml grep] |
| Rust 标准库 `u8::to_ascii_uppercase()` | stable | SQL 关键字大写化 | [ASSUMED] 无需额外 crate |

**不引入新 crate。** 所有功能可用已有基础设施实现。

### 无需安装

本 phase 无外部包安装步骤，跳过 Package Legitimacy Audit。

---

## Architecture Patterns

### System Architecture Diagram

```
sql_text (&str)
    │
    ▼
normalize_template(sql)        ← src/features/sql_fingerprint.rs
    │
    ├─ scan loop (单遍字节扫描)
    │      ├─ 遇到单引号 '  → memchr 跳至闭合引号（字面量原文保留）
    │      ├─ 遇到 --        → 跳至行尾（注释去除，D-16）
    │      ├─ 遇到 /*        → memchr 搜索 */（多行注释去除，替换为空格，D-16）
    │      ├─ 遇到标识符/关键字 → is_keyword() 判断；是则大写化输出（D-04）
    │      └─ 遇到空白        → 折叠为单空格（继承自 fingerprint() 模式）
    │
    ├─ in_list_fold(out: &[u8]) → 后处理：扫描 "IN (" 标记，折叠内容为 "?"（D-01/D-02）
    │
    └─ String（template key）
           │
           ▼
    cli/run.rs 热循环
           │
           └─ if do_template { let tmpl_key = normalize_template(pm.sql.as_ref()); }
                   暂存为局部变量（D-14）
```

### Recommended Project Structure（变更最小化）

```
src/
├── features/
│   ├── sql_fingerprint.rs   ← 新增 normalize_template() + 共享扫描基础（D-06/D-07）
│   └── mod.rs               ← 新增 pub use sql_fingerprint::normalize_template（D-09）
│                              新增 TemplateAnalysisConfig 结构体（D-10/D-11）
│                              FeaturesConfig 新增 template_analysis 字段（D-11）
└── cli/
    └── run.rs               ← 新增 do_template 变量 + 条件调用（D-13）
```

---

## Pattern 1: 单遍字节扫描（继承 fingerprint() 结构）

**What:** 主循环用 `NEEDS_SPECIAL` 表跳过普通字节批量复制，仅在特殊字节处分发。
**When to use:** 所有文本变换逻辑均应走此路径。

```rust
// Source: src/features/sql_fingerprint.rs（现有模式）
while i < len {
    let bulk_start = i;
    while i < len && !NEEDS_SPECIAL[bytes[i] as usize] {
        i += 1;
    }
    if i > bulk_start {
        out.extend_from_slice(&bytes[bulk_start..i]);
    }
    if i >= len { break; }

    match bytes[i] {
        b'\'' => { /* memchr 跳字面量 */ }
        // normalize_template 新增：
        b'-' if i + 1 < len && bytes[i + 1] == b'-' => { /* 跳至行尾 */ }
        b'/' if i + 1 < len && bytes[i + 1] == b'*' => { /* 跳至 */ */ }
        b if b.is_ascii_alphabetic() => { /* 关键字检测 + 大写化 */ }
        // ...
    }
}
```

**注意：** 要为 `normalize_template` 扩展 `NEEDS_SPECIAL` 表，需将 `-`（`0x2D`）和 `/`（`0x2F`）也标记为特殊字节，因为它们是注释起始的第一个字节。

---

## Pattern 2: 关键字匹配（字节比较，不用 regex）

**What:** 在标识符边界处检查当前单词是否属于关键字集合，是则大写化输出。
**When to use:** 扫描到字母字节时触发。

```rust
// [ASSUMED] 参考模式 — 实际 impl 由 planner 决定具体边界检查方式
fn is_keyword(word: &[u8]) -> bool {
    // 关键字集合：phf_set! 或简单的 match
    matches!(word,
        b"select" | b"SELECT" | b"from" | b"FROM" | b"where" | b"WHERE"
        | b"and" | b"AND" | b"or" | b"OR" | b"join" | b"JOIN"
        | b"on" | b"ON" | b"as" | b"AS" | b"insert" | b"INSERT"
        | b"update" | b"UPDATE" | b"delete" | b"DELETE" | b"group" | b"GROUP"
        | b"order" | b"ORDER" | b"by" | b"BY" | b"having" | b"HAVING"
        | b"union" | b"UNION" | b"distinct" | b"DISTINCT" | b"limit" | b"LIMIT"
        | b"create" | b"CREATE" | b"drop" | b"DROP" | b"alter" | b"ALTER"
        | b"into" | b"INTO" | b"values" | b"VALUES" | b"set" | b"SET"
        | b"in" | b"IN" | b"not" | b"NOT" | b"null" | b"NULL"
        | b"is" | b"IS" | b"between" | b"BETWEEN" | b"like" | b"LIKE"
        | b"exists" | b"EXISTS" | b"case" | b"CASE" | b"when" | b"WHEN"
        | b"then" | b"THEN" | b"else" | b"ELSE" | b"end" | b"END"
    )
}
```

**关键字识别边界：** 一个单词是否为关键字，必须在完整单词边界（前后不是 `is_ident_byte`）时才大写化，以避免把标识符 `username` 中的 `name` 误匹配。

---

## Pattern 3: TemplateAnalysisConfig（仿照 ReplaceParametersConfig）

**What:** 最小化配置结构，只含 `enabled: bool`。

```rust
// Source: src/features/mod.rs（ReplaceParametersConfig 现有模式，直接仿照）
#[derive(Debug, Deserialize, Clone)]
pub struct TemplateAnalysisConfig {
    #[serde(default)]   // 默认 false（D-12）
    pub enabled: bool,
}

impl Default for TemplateAnalysisConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}
```

**FeaturesConfig 新增字段：**

```rust
pub struct FeaturesConfig {
    pub filters: Option<FiltersFeature>,
    pub replace_parameters: Option<ReplaceParametersConfig>,
    pub fields: Option<Vec<String>>,
    pub template_analysis: Option<TemplateAnalysisConfig>,  // 新增（D-11）
}
```

---

## Pattern 4: 热循环条件调用（仿照 do_normalize 模式）

**What:** 在 `handle_run` 入口提前计算 `do_template` bool，热循环内用 `if` 守卫。

```rust
// Source: src/cli/run.rs handle_run()（现有 do_normalize 模式，直接仿照）

// 在 handle_run() 中，pipeline 构建后：
let do_template = final_cfg
    .features
    .template_analysis
    .as_ref()
    .is_some_and(|t| t.enabled);

// process_log_file() 新增参数：do_template: bool
// 在热循环内：
let tmpl_key: Option<String> = if do_template {
    Some(crate::features::normalize_template(pm.sql.as_ref()))
} else {
    None
};
// tmpl_key 暂存为局部变量供 Phase 13 使用（D-14）
```

**零开销保证：** `do_template = false` 时 `normalize_template()` 完全不调用，分支预测器将条件标记为冷路径，无 allocator 压力。

---

## Pattern 5: IN 列表折叠（后处理策略）

**What:** 在主扫描循环中识别 `IN` 关键字后，向前扫描括号内容并折叠。
**When to use:** 遇到 `IN (` 序列时触发（关键字 IN 已大写化，可用固定字节比较）。

两种可行策略：

**策略 A（主循环内联）：** 识别 `IN` 关键字时，不立即写入 `IN`，而是向前消费完整 `(...)` 内容，输出 `IN (?)`。
- 优点：单遍完成，无二次 buffer 扫描
- 缺点：主循环状态机更复杂

**策略 B（后处理）：** 主循环正常输出（含 `IN (1, 2, 3)`），后处理 pass 用正则或字节扫描折叠 `IN (...)` 内容。
- 优点：主循环保持简洁
- 缺点：需要二次扫描 output buffer，增加 allocation

**推荐策略 A**，理由：`IN` 是已确认关键字（大写化后为 `IN`，4 字节 match 即可），直接在主循环检测并消费括号内容，维持单遍性能特征，与 `fingerprint()` 的 `'` 字面量内联跳过一致。

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SIMD 字节搜索（`*/` 终止符） | 手写 memchr 等价物 | `memchr::memmem::find(bytes, b"*/")` | 已是依赖，Two-Way+SIMD，经验证的正确性 |
| SQL 关键字集合（case-insensitive lookup） | HashMap<String, bool> | 字节级 `match` 或 `phf` 静态 set | match 无 hash 开销；phf 如需引入需通过 slopcheck |
| UTF-8 合法性保证（输出 buffer） | 手写 UTF-8 校验 | 继承 `fingerprint()` 的已有论证：注释去除/大写化只操作 ASCII 字节（0x00–0x7F），不破坏多字节序列 | [ASSUMED] 同 fingerprint() 的 UTF-8 不变量分析 |

**关键洞察：** `fingerprint()` 已证明字节级操作可维护 UTF-8 合法性（仅操作 ASCII 字节，不拆断多字节序列），`normalize_template()` 遵循相同约束。

---

## Common Pitfalls

### Pitfall 1: 字符串字面量内注释符号被误处理
**What goes wrong:** `WHERE comment = '-- not a comment'` 中的 `--` 被去除。
**Why it happens:** 主循环先遇到 `-` 字节，未检查是否在字面量内。
**How to avoid:** 字符串字面量跳过逻辑（`'...'` 内循环）必须先于注释检测执行。在 `NEEDS_SPECIAL` 中 `'` 优先级最高，字面量内所有字节（包括 `--`、`/*`）都在 memchr 跳过时绕过主分发。
**Warning signs:** 测试用例 `WHERE a = '--'` → 输出应保留 `WHERE a = '--'` 不变。

### Pitfall 2: `IN` 关键字误识别（非独立单词）
**What goes wrong:** 标识符 `CONTAINS`、列名 `IN_STOCK` 中的 `IN` 被当作 IN 列表起始。
**Why it happens:** 未做单词边界检查。
**How to avoid:** 检测到 `IN` 后，必须验证前一个字节是非标识符字节（空格、`(`、行首），且后接 `(` 之前只有空白。
**Warning signs:** 测试用例 `WHERE status IN_STOCK = 1` 不应折叠任何内容。

### Pitfall 3: 多行注释跨越字符串字面量边界
**What goes wrong:** `/* comment ' with quote */` 中的单引号触发字面量解析模式，导致注释未被正确去除。
**Why it happens:** 注释内的引号被主循环误判为字面量开始。
**How to avoid:** 进入 `/* */` 注释块后，直接用 `memchr::memmem::find(bytes, b"*/")` 一次性跳到注释结尾，不经过主分发逻辑。注释内的所有字节（包括引号）被原子性跳过。
**Warning signs:** 测试用例 `/* say 'hi' */ SELECT 1` → 输出应为 `SELECT 1`（注释被替换为单空格）。

### Pitfall 4: 关键字大写化破坏 `OUT` 变量名识别
**What goes wrong:** Oracle 风格的 `:OUT` 参数或列名 `outer_join` 中的 `OUTER` 部分大写化产生 `OUTER_join`。
**Why it happens:** 关键字边界检测不够严格。
**How to avoid:** 使用 `prev_is_ident_byte(out)` 和 `bytes[i + word_len]` 双向边界检查，确保关键字前后均为非标识符字节。
**Warning signs:** 测试用例 `outer_key = 1` 中 `outer` 不应大写化（后接 `_` 为标识符字节）。

### Pitfall 5: NEEDS_SPECIAL 扩展遗漏 `-` 和 `/`
**What goes wrong:** 注释起始字节 `-`（`0x2D`）和 `/`（`0x2F`）未在 `NEEDS_SPECIAL` 中标记，被批量复制路径跳过，注释从未被检测到。
**Why it happens:** 现有 `NEEDS_SPECIAL` 只覆盖 `'`、空白、数字。
**How to avoid:** 扩展 `NEEDS_SPECIAL` 常量，将 `b'-'` 和 `b'/'` 也设为 `true`。这是 `normalize_template` 与 `fingerprint()` 的主要差异点之一。
**Warning signs:** 单元测试 `-- comment\nSELECT 1` 未去除注释时立即暴露。

---

## Code Examples

### 注释去除：单行 `--`
```rust
// [ASSUMED] 基于 fingerprint() 结构的扩展模式
b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
    // 跳至行尾或 EOF（D-16）
    if let Some(rel) = memchr::memchr(b'\n', &bytes[i..]) {
        i += rel + 1; // 跳过 \n 本身
    } else {
        i = len;
    }
    // 不向 out 写入任何内容
}
```

### 注释去除：多行 `/* */`
```rust
// [ASSUMED] 基于 memchr::memmem 的注释块跳过
b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
    i += 2; // 跳过 /*
    if let Some(rel) = memchr::memmem::find(&bytes[i..], b"*/") {
        i += rel + 2; // 跳过 */
    } else {
        i = len; // 未闭合注释：跳到末尾
    }
    // 替换为单空格（D-16），避免两侧 token 粘连
    if !matches!(out.last(), Some(&b' ')) {
        out.push(b' ');
    }
}
```

### TemplateAnalysisConfig 的 TOML 示例
```toml
[features.template_analysis]
enabled = true
```

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test（cargo test） |
| Config file | 无独立配置文件 |
| Quick run command | `cargo test -p dm-database-sqllog2db features::sql_fingerprint` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TMPL-01 | 注释去除（`--` 和 `/* */`） | unit | `cargo test normalize_template` | ❌ Wave 0 |
| TMPL-01 | IN 列表折叠（数字列表） | unit | `cargo test normalize_template` | ❌ Wave 0 |
| TMPL-01 | IN 列表折叠（字符串列表） | unit | `cargo test normalize_template` | ❌ Wave 0 |
| TMPL-01 | 关键字大写化 | unit | `cargo test normalize_template` | ❌ Wave 0 |
| TMPL-01 | 空白折叠 | unit | `cargo test normalize_template` | ❌ Wave 0 |
| TMPL-01 | 字面量内注释符不误判（D-15） | unit | `cargo test normalize_template` | ❌ Wave 0 |
| TMPL-01 | 相同语义不同参数产生相同 key（D-03） | unit | `cargo test normalize_template` | ❌ Wave 0 |
| TMPL-01 | TemplateAnalysisConfig serde 反序列化 | unit | `cargo test template_analysis_config` | ❌ Wave 0 |
| TMPL-01 | FeaturesConfig 默认无 template_analysis | unit | `cargo test features_config` | ❌ Wave 0（需扩展已有测试） |
| TMPL-01 | clippy 零 warning | lint | `cargo clippy --all-targets -- -D warnings` | ✅ 已有 CI 路径 |

### Sampling Rate
- **Per task commit:** `cargo test -p dm-database-sqllog2db`（~0.02s 全量）
- **Per wave merge:** `cargo clippy --all-targets -- -D warnings && cargo test`
- **Phase gate:** 两者全绿 + 无新 clippy warning

### Wave 0 Gaps
- [ ] `src/features/sql_fingerprint.rs` 新增 `normalize_template()` 单元测试模块（或扩展现有 `mod tests`）
- [ ] `src/features/mod.rs` 新增 `TemplateAnalysisConfig` 单元测试（参照 `test_replace_parameters_config_default` 模式）

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 无模板归一化 | normalize_template() 四项变换 | Phase 12（本阶段） | 为 Phase 13 的统计聚合提供稳定 key |
| fingerprint() 独立扫描 | 与 normalize_template() 共享 NEEDS_SPECIAL + 扫描引擎 | Phase 12（本阶段） | 减少重复代码，保持单一事实来源 |

---

## Open Questions (RESOLVED)

1. **IN 折叠的嵌套括号处理**
   - What we know: DM SQL 日志中 IN 列表通常为简单平面列表（`IN (1, 2, 3)`）
   - What's unclear: 是否存在 `IN (SELECT ...)` 子查询需要排除折叠？
   - Recommendation: 先实现简单情形（括号内仅含字面量），若遇到 `SELECT`/`FROM` 关键字则跳过折叠，保留原文。可在 Wave 0 测试中覆盖此边界情形。

2. **共享扫描引擎的抽象边界**
   - What we know: D-07 要求抽取 `scan_sql_bytes()` 让两个函数共享，D-08 要求通过参数化区分行为
   - What's unclear: 具体用泛型闭包 `F: FnMut(...)` 还是内部 enum 策略？
   - Recommendation: 用枚举 `enum ScanMode { Fingerprint, Normalize }` 最简单，避免单态化膨胀，同时保持 `fingerprint()` 现有的内联优化。Claude 的裁量权范围内（详见 CONTEXT.md `<specifics>`）。

---

## Environment Availability

Step 2.6: SKIPPED（本 phase 为纯代码变更，无外部工具依赖）

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `u8::to_ascii_uppercase()` 足以大写化 SQL 关键字（所有关键字为纯 ASCII） | Code Examples / Pattern 2 | 极低风险：SQL 关键字定义为 ASCII，DM SQL 保留字同样为 ASCII |
| A2 | 关键字识别采用 `match` 字节切片，无需引入 `phf` crate | Pattern 2 | 若关键字集合超过 ~50 个且有性能回归，可引入 phf；目前预估 ~30 个关键字，match 足够 |
| A3 | `normalize_template()` 返回类型为 `String`（与 `fingerprint()` 一致）而非 `CompactString` | Pattern 4 | 若模板 key 通常 < 24 字节，CompactString 可减少堆分配；但 Phase 13 observe() 接口需对齐，保守选 String |
| A4 | IN 列表折叠在主循环内联（策略 A）优于后处理（策略 B） | Pattern 5 | 若实现复杂度过高，可退回策略 B；对 Phase 12 正确性无影响，仅影响性能 |

**Assumptions 数量较少且风险均为低/极低。所有核心设计均由 CONTEXT.md 锁定决策驱动，无高风险假设。**

---

## Sources

### Primary（HIGH confidence，代码库直接检查）
- `src/features/sql_fingerprint.rs` — 现有 fingerprint() 完整实现；NEEDS_SPECIAL 表；memchr 字符串跳过模式 [VERIFIED: codebase read]
- `src/features/replace_parameters.rs` — 字符串字面量跳过 + memchr2 主循环模式 [VERIFIED: codebase read]
- `src/features/mod.rs` — ReplaceParametersConfig 完整模式；FeaturesConfig 结构 [VERIFIED: codebase read]
- `src/cli/run.rs` — do_normalize 条件调用完整模式（第 698-703 行）；process_log_file 参数传递 [VERIFIED: codebase read]
- `src/config.rs` — Config 结构；apply_one() 已有 features.replace_parameters.enable 路径 [VERIFIED: codebase read]
- `Cargo.toml` — memchr 2.x 已是依赖；无需新增 crate [VERIFIED: codebase read]

### Secondary（MEDIUM confidence）
- `.planning/phases/12-sql/12-CONTEXT.md` — 所有 D-01 至 D-16 决策 [VERIFIED: planning docs read]
- `.planning/REQUIREMENTS.md` — TMPL-01 需求定义 [VERIFIED: planning docs read]

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — 无新依赖，所有库已在 Cargo.toml 验证
- Architecture: HIGH — 直接参照现有 fingerprint()/ReplaceParametersConfig/do_normalize 模式，代码已读
- Pitfalls: HIGH — 基于代码库实际扫描逻辑推导，覆盖字面量边界/关键字边界/注释边界三类主要陷阱
- Test map: HIGH — 直接映射 TMPL-01 的 4 项成功标准到可自动化的 cargo test 命令

**Research date:** 2026-05-15
**Valid until:** 稳定（无外部依赖），代码结构不变则持续有效

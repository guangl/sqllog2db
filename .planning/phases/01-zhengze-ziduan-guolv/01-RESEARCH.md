# Phase 1: 正则字段过滤 - Research

**Researched:** 2026-04-18
**Domain:** Rust regex filtering — `regex` crate, `FiltersFeature` / `MetaFilters` 修改
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** 升级现有字段支持正则。`usernames`, `client_ips`, `appnames`, `tags`, `sess_ids`, `thrd_ids`, `statements` 字段直接接受正则字符串，配置格式不变，向后兼容。
- **D-02:** 同一字段列表内多个正则是 **OR** 语义——任意一个正则匹配即满足该字段。
- **D-03:** `record_sql` 的 `include_patterns`/`exclude_patterns` 字段升级为正则匹配，在记录级主循环中判断。事务级 `sql` 过滤（预扫描）保持字符串包含匹配，本阶段不升级。
- **D-04:** 跨字段是 **AND** 语义——所有配置了过滤条件的字段必须同时满足，记录才被保留。替换现有 `should_keep()` 中跨字段 OR 的逻辑。

### Claude's Discretion

- 正则在配置加载后、进入热循环前编译（`FiltersFeature::from_config()` 或类似构造时）。
- 实现方式（`regex::Regex` 直接编译 vs `regex::RegexSet`）由 Claude 决定。
- 启动阶段报错而非运行时。

### Deferred Ideas (OUT OF SCOPE)

- 事务级 `sql` 过滤（预扫描）的正则升级
- FILTER-03 排除模式
- OR 条件组合
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FILTER-01 | 用户可对任意字段配置正则表达式过滤条件，运行时仅保留所有正则均匹配的记录 | `regex::Regex::is_match()` 替换 `str::contains()`；`MetaFilters` 存储预编译 `Regex` 列表 |
| FILTER-02 | 多个过滤条件列表默认 AND 语义 | `MetaFilters::should_keep()` 改为跨字段全部满足才返回 true；字段内保持 OR |
</phase_requirements>

---

## Summary

本阶段在现有 `FiltersFeature` / `MetaFilters` / `SqlFilters` 结构上做最小外科手术：

1. 引入 `regex = "1.12.3"` 依赖（[VERIFIED: cargo search]）。
2. `MetaFilters` 中的 `Vec<String>` 字段在构造时预编译为 `Vec<regex::Regex>`，存储在 `CompiledMetaFilters` 结构（或 `MetaFilters` 本身新增 `compiled_*` 字段）。
3. `MetaFilters::should_keep()` 改为跨字段 AND：每个配置了过滤的字段都必须匹配，否则返回 false。字段内仍然是 OR（any 匹配）。
4. `SqlFilters` 的 `include_patterns` / `exclude_patterns` 升级为正则，`SqlFilters` 同样存储 `Vec<regex::Regex>`。
5. 正则编译在 `Config::validate()` 中触发（或在 `build_pipeline` 构造 `FilterProcessor` 时），失败立即返回 `ConfigError::InvalidValue`。

**Primary recommendation:** 使用 `Vec<regex::Regex>`（每个字段独立的 Regex 列表）而非 `RegexSet`。理由见下方架构章节。

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 正则编译与验证 | Config 层 (`validate()`) | `FilterProcessor::new()` | 启动阶段报错，不在热路径中编译 |
| 字段内 OR 匹配 | `MetaFilters::should_keep()` | — | 单字段多正则，any() 即可 |
| 跨字段 AND 组合 | `MetaFilters::should_keep()` | — | 替换现有 OR 逻辑 |
| SQL 记录级正则 | `SqlFilters::matches()` 热路径 | `run.rs` process_log_file | `record_sql` 在记录循环中调用 |
| 快路径保护 | `FilterProcessor.has_meta_filters` 预计算标志 | `pipeline.is_empty()` | 未配置过滤时零开销 |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `regex` | 1.12.3 | 正则表达式编译与匹配 | Rust 生态标准，线性时间保证，官方维护 |

[VERIFIED: cargo search regex —— 最新版 1.12.3，2024 年发布]

**当前 Cargo.toml 未包含 `regex`，需要新增。**

### 无需引入的库

| 需求 | 结论 |
|------|------|
| `lazy-regex` | 不需要。正则在启动时统一预编译，无需 lazy static |
| `regex::RegexSet` | 不推荐用于本场景（见下方 Architecture Patterns） |

**Installation:**
```bash
cargo add regex
# 或手动在 Cargo.toml [dependencies] 添加：
# regex = "1.12.3"
```

**Version verification:** `cargo search regex --limit 1` 确认当前为 1.12.3。[VERIFIED: cargo search]

---

## Architecture Patterns

### System Architecture Diagram

```
Config::from_file() → Config::validate()
         │
         ├─ 正则字符串格式验证（FiltersFeature::validate_regexes）
         │   └─ Regex::new(pattern) → Err → ConfigError::InvalidValue → 启动中止
         │
         └─ 返回 Config (字符串仍存在 meta / sql 字段)
                  │
                  ▼
         build_pipeline(cfg)
                  │
                  └─ FilterProcessor::new(filters_feature)
                           │
                           ├─ CompiledMetaFilters::from(&meta_filters) → Vec<Regex> 预编译
                           ├─ compiled_record_sql: CompiledSqlFilters::from(&record_sql)
                           └─ has_meta_filters: bool (预计算)
                                    │
                        ─────────────────────────────
                        热路径（每条记录）
                        ─────────────────────────────
                                    │
                        pipeline.is_empty() ? → 快路径跳过
                                    │
                        process_with_meta(record, meta)
                           │
                           ├─ 时间过滤（字符串比较，无变化）
                           │
                           ├─ has_meta_filters? No → return true
                           │
                           └─ CompiledMetaFilters::should_keep(&RecordMeta)
                               │
                               ├─ usernames 配置了? → any(|re| re.is_match(user))  → false → return false  [AND]
                               ├─ client_ips 配置了? → any(|re| re.is_match(ip))   → false → return false  [AND]
                               ├─ sess_ids   配置了? → any(|re| re.is_match(sess))  → false → return false  [AND]
                               ├─ ... (其余字段同理)
                               └─ 全部通过 → return true
                                    │
                        sql_record_filter (CompiledSqlFilters::matches)
                           ├─ include_patterns: any(|re| re.is_match(sql)) → false → 跳过导出
                           └─ exclude_patterns: any(|re| re.is_match(sql)) → true  → 跳过导出
```

### Recommended Project Structure

无需新增模块。所有变更集中在：

```
src/
├── features/
│   └── filters.rs     # 主要改动：MetaFilters / SqlFilters 正则化 + 新 Compiled* 结构
├── config.rs          # validate() 新增正则验证入口
└── cli/
    └── run.rs         # FilterProcessor::new() 调用正则预编译（若编译放在此处）
```

### Pattern 1: `Vec<regex::Regex>` vs `regex::RegexSet`

**选择 `Vec<regex::Regex>`，原因：**

- `RegexSet` 用于"同时匹配多个 pattern，返回哪些 pattern 命中"的场景。本场景只需要 `any()` 语义（OR），`Vec<Regex>` + `.iter().any(|re| re.is_match(val))` 完全够用。
- `RegexSet::is_match()` 会对所有 pattern 都执行，而 `Vec<Regex>` + `any()` 在第一个匹配后短路，对短列表（典型配置 1-5 个）更高效。
- `RegexSet` 不能返回捕获组，未来如需扩展时有局限性。
- `Vec<Regex>` 实现更简单，与现有 `Vec<String>` 结构对应清晰。

[VERIFIED: Context7 /rust-lang/regex 文档]

**实现方式：两套并行结构**

`MetaFilters` 保持 `serde` Deserialize 结构（存储原始字符串），新增 `CompiledMetaFilters` 存储预编译结果：

```rust
// Source: 架构设计，基于 regex crate 1.12.3 API
use regex::Regex;

/// 预编译后的元数据过滤器，热路径使用
#[derive(Debug)]
pub struct CompiledMetaFilters {
    pub usernames:  Option<Vec<Regex>>,
    pub client_ips: Option<Vec<Regex>>,
    pub sess_ids:   Option<Vec<Regex>>,
    pub thrd_ids:   Option<Vec<Regex>>,
    pub statements: Option<Vec<Regex>>,
    pub appnames:   Option<Vec<Regex>>,
    pub tags:       Option<Vec<Regex>>,
    // trxids 保持 TrxidSet（精确匹配，不用正则）
    pub trxids:     Option<TrxidSet>,
}

impl CompiledMetaFilters {
    pub fn from_config(meta: &MetaFilters) -> Result<Self, (String, String)> {
        // 逐字段编译，失败时返回 (field_name, pattern)
        Ok(Self {
            usernames:  compile_patterns(meta.usernames.as_deref())?,
            client_ips: compile_patterns(meta.client_ips.as_deref())?,
            // ...
        })
    }

    #[inline]
    #[must_use]
    pub fn should_keep(&self, meta: &RecordMeta) -> bool {
        // AND 语义：每个有配置的字段都必须通过
        if !match_any_regex(self.usernames.as_deref(), meta.user)  { return false; }
        if !match_any_regex(self.client_ips.as_deref(), meta.ip)   { return false; }
        if !match_any_regex(self.sess_ids.as_deref(), meta.sess)   { return false; }
        if !match_any_regex(self.thrd_ids.as_deref(), meta.thrd)   { return false; }
        if !match_any_regex(self.statements.as_deref(), meta.stmt) { return false; }
        if !match_any_regex(self.appnames.as_deref(), meta.app)    { return false; }
        // trxids 精确匹配（不变）
        if let Some(trxids) = &self.trxids {
            if !trxids.is_empty() && !trxids.contains(meta.trxid) { return false; }
        }
        // tags 特殊：字段可能为 None
        if let Some(tag_patterns) = &self.tags {
            match meta.tag {
                Some(t) if !tag_patterns.iter().any(|re| re.is_match(t)) => return false,
                None if !tag_patterns.is_empty() => return false,
                _ => {}
            }
        }
        true
    }
}

/// 编译一组正则字符串，任一失败返回 Err((field, pattern))
fn compile_patterns(
    patterns: Option<&Vec<String>>,
) -> Result<Option<Vec<Regex>>, (String, String)> {
    match patterns {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(v) => {
            let compiled = v
                .iter()
                .map(|p| Regex::new(p).map_err(|_| p.clone()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|bad_pattern| ("(field)".to_string(), bad_pattern))?;
            Ok(Some(compiled))
        }
    }
}

/// 辅助：Option<&[Regex]> 对 val 的 OR 匹配
/// None → 该字段未配置过滤，视为"通过"
#[inline]
fn match_any_regex(patterns: Option<&[Regex]>, val: &str) -> bool {
    match patterns {
        None => true,   // 未配置：不参与 AND 判断，直接通过
        Some(p) => p.iter().any(|re| re.is_match(val)),
    }
}
```

### Pattern 2: 正则验证在 `Config::validate()` 中触发

```rust
// Source: 现有 config.rs validate() 模式
impl Config {
    pub fn validate(&self) -> Result<()> {
        // 已有验证...
        if let Some(filters) = &self.features.filters {
            if filters.enable {
                filters.validate_regexes()?;
            }
        }
        Ok(())
    }
}

impl FiltersFeature {
    pub fn validate_regexes(&self) -> Result<()> {
        validate_pattern_list("features.filters.usernames",
            self.meta.usernames.as_deref())?;
        validate_pattern_list("features.filters.client_ips",
            self.meta.client_ips.as_deref())?;
        // ... 其余字段
        validate_pattern_list("features.filters.record_sql.include_patterns",
            self.record_sql.include_patterns.as_deref())?;
        validate_pattern_list("features.filters.record_sql.exclude_patterns",
            self.record_sql.exclude_patterns.as_deref())?;
        Ok(())
    }
}

fn validate_pattern_list(field: &str, patterns: Option<&Vec<String>>) -> Result<()> {
    if let Some(list) = patterns {
        for pattern in list {
            Regex::new(pattern).map_err(|e| {
                Error::Config(ConfigError::InvalidValue {
                    field: field.to_string(),
                    value: pattern.clone(),
                    reason: format!("invalid regex: {e}"),
                })
            })?;
        }
    }
    Ok(())
}
```

### Pattern 3: `FilterProcessor` 存储预编译结构

```rust
#[derive(Debug)]
struct FilterProcessor {
    compiled_meta:       CompiledMetaFilters,
    compiled_record_sql: CompiledSqlFilters,   // include / exclude 已是 Vec<Regex>
    has_meta_filters:    bool,
    // 时间范围直接复用原始字符串
    start_ts: Option<String>,
    end_ts:   Option<String>,
}

impl FilterProcessor {
    fn new(filter: FiltersFeature) -> Self {
        let compiled_meta = CompiledMetaFilters::from_config(&filter.meta)
            .expect("regexes already validated in Config::validate()");
        // ...
    }
}
```

### Anti-Patterns to Avoid

- **运行时编译正则：** 在 `should_keep()` 热路径中调用 `Regex::new()` — 每条记录都会重新编译，性能灾难。
- **使用 `RegexSet` 做 OR 匹配：** `RegexSet::is_match()` 不支持短路，对小列表反而慢。
- **`unwrap()` 替代错误处理：** 验证阶段已保证正则合法，`FilterProcessor::new()` 中 `expect()` 可接受，但不要在测试外对用户输入 `unwrap()`。
- **跨字段 OR 遗留：** 现有 `MetaFilters::should_keep()` 是 OR；修改后若漏掉某个字段的 AND 转换，会静默地给出错误结果。

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 正则解析与匹配 | 自定义 NFA/DFA | `regex::Regex` | Unicode 支持、线性时间保证、缓存 DFA 状态机 |
| 错误格式化 | 手拼字符串 | `thiserror` + `ConfigError::InvalidValue` | 现有错误体系，保持一致 |

---

## Common Pitfalls

### Pitfall 1: AND/OR 语义翻转——漏掉字段

**What goes wrong:** `should_keep()` 中新字段加了 AND 逻辑，但旧字段（如 `trxids`）还是 OR，导致不一致。
**Why it happens:** `trxids` 语义特殊（预扫描注入），容易忽略。
**How to avoid:** 明确区分：`trxids` 由预扫描保证，正式扫描时如果 trxids 有值就精确匹配；AND 框架中 trxids 视为另一个"已配置字段"，必须通过。
**Warning signs:** 测试用例"同时配置 usernames 和 trxids，不满足 trxids 的记录被保留"时测试不失败。

### Pitfall 2: `has_meta_filters` 预计算未同步更新

**What goes wrong:** `FilterProcessor::new()` 中 `has_meta_filters = filter.meta.has_filters()` 计算的是原始 `MetaFilters` 的结果，若 `CompiledMetaFilters` 与之不同步会引发快路径误判。
**Why it happens:** 重构后 `MetaFilters::has_filters()` 仍检查原始字符串，`CompiledMetaFilters` 是新结构，忘记用 compiled 的版本重新算。
**How to avoid:** `has_meta_filters` 应基于 `CompiledMetaFilters` 的实际编译结果（是否有非 None 字段）来计算，而非调用旧 `MetaFilters::has_filters()`。

### Pitfall 3: `None` 字段的 AND 语义混淆

**What goes wrong:** `usernames = None`（未配置）时，`match_any_regex(None, val)` 应返回 `true`（"不参与过滤"），实现成返回 `false`（"默认拒绝"）。
**Why it happens:** AND 语义中容易把"未配置字段"与"配置了空列表"混淆。
**How to avoid:** `None` → 不参与过滤，return `true`；`Some([])` → 同 `None`（`compile_patterns` 将空列表转为 `None`）。

### Pitfall 4: `record_sql` 正则升级但 `sql`（事务级）未升级

**What goes wrong:** 同为 `SqlFilters` 类型，`record_sql` 升级了正则，`sql`（预扫描 `scan_log_file_for_matches`）却还是 `str::contains()`，造成行为不一致。
**Why it happens:** D-03 明确仅升级 `record_sql`，但代码复用同一结构，改 `SqlFilters::matches()` 会同时影响两者。
**How to avoid:** `SqlFilters` 结构一分为二（`RawSqlFilters` / `CompiledSqlFilters`），或在 `scan_log_file_for_matches` 中继续调用旧的字符串包含匹配，而 `FilterProcessor` 只持有 `CompiledSqlFilters`。最清晰的方案是让事务级扫描直接用原始字符串，记录级使用编译好的正则结构。

### Pitfall 5: `tags` 字段的 `Option<&str>` 特殊处理

**What goes wrong:** `meta.tag` 是 `Option<&str>`（有些记录无 tag），若未配置 `tags` 过滤但 tag 为 `None` 时错误地拒绝记录。
**Why it happens:** AND 框架中 tag 需要额外区分"字段未配置"与"字段值为 None"。
**How to avoid:** `tags = None`（未配置）→ 跳过，return `true`；`tags = Some([patterns])`，`meta.tag = None` → 无法匹配任何 pattern，return `false`（有配置但记录无 tag，不满足条件）。

---

## Code Examples

### 正则编译与错误处理（启动阶段）

```rust
// 基于 regex 1.12.3 API + 现有 ConfigError 模式
use regex::Regex;
use crate::error::{ConfigError, Error, Result};

fn validate_pattern_list(field: &str, patterns: Option<&Vec<String>>) -> Result<()> {
    let Some(list) = patterns else { return Ok(()) };
    for pattern in list {
        Regex::new(pattern).map_err(|e| Error::Config(ConfigError::InvalidValue {
            field: field.to_string(),
            value: pattern.clone(),
            reason: format!("invalid regex: {e}"),
        }))?;
    }
    Ok(())
}
```

### AND 语义的 `should_keep` 核心逻辑

```rust
// 关键：None 表示"未配置，通过"；Some(patterns) 表示"必须匹配其中之一"
#[inline]
fn match_any_regex(patterns: Option<&[Regex]>, val: &str) -> bool {
    match patterns {
        None => true,
        Some(p) if p.is_empty() => true,
        Some(p) => p.iter().any(|re| re.is_match(val)),
    }
}
```

### `SqlFilters` 升级为正则（记录级）

```rust
// CompiledSqlFilters 仅用于 record_sql（记录级），事务级 sql 继续用字符串包含
#[derive(Debug)]
pub struct CompiledSqlFilters {
    pub include_patterns: Option<Vec<Regex>>,
    pub exclude_patterns: Option<Vec<Regex>>,
}

impl CompiledSqlFilters {
    #[must_use]
    pub fn matches(&self, sql: &str) -> bool {
        // include：必须命中其中之一（未配置 = 通过）
        let include_ok = self.include_patterns.as_deref()
            .map_or(true, |p| p.is_empty() || p.iter().any(|re| re.is_match(sql)));
        if !include_ok { return false; }
        // exclude：不能命中任何一个
        if let Some(excl) = &self.exclude_patterns {
            if excl.iter().any(|re| re.is_match(sql)) { return false; }
        }
        true
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `str::contains()` 子串匹配 | `regex::Regex::is_match()` 正则匹配 | Phase 1 | 用户可用 `^`, `$`, `.*`, `\d+` 等模式；纯字符串仍向后兼容 |
| 跨字段 OR 语义 | 跨字段 AND 语义 | Phase 1 | **Breaking change（语义层面）**：同时配置多个字段时，行为与旧版不同 |

**Breaking change 说明：** 旧版 `should_keep()` 是"任意元数据字段命中即保留"，新版是"所有配置了的字段都必须命中才保留"。现有用户如果只配置了一个字段，行为不变；配置多个字段的用户会发现结果集更小（更精确）——这正是 FILTER-02 的目标。

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `regex::Regex::is_match()` 对典型配置（1-5 个 pattern）比 `RegexSet::is_match()` 更快，因为支持短路 | Architecture Patterns | 影响建议的实现方式；实际差异对小列表很小，两种方案都可行 |

---

## Open Questions

1. **`CompiledMetaFilters` 放在 `filters.rs` 还是保持 `MetaFilters` 扩展？**
   - 什么我们知道：`MetaFilters` 是 `serde` Deserialize 结构，不适合直接存 `Regex`（Regex 不实现 Deserialize）
   - 什么不清楚：是在 `MetaFilters` 旁边定义独立 `CompiledMetaFilters`，还是让 `FiltersFeature` 在构造时持有两套字段
   - Recommendation：新增独立 `CompiledMetaFilters` 结构，`FilterProcessor` 只持有编译后的版本，`MetaFilters` 原始结构保留用于 serde

2. **`trxids` 在 AND 框架中的语义边界**
   - 什么我们知道：`trxids` 由预扫描注入，不是用户在 config 里直接配置的
   - 什么不清楚：AND 语义下，预扫描注入的 `trxids` 是"参与 AND"还是"等同于通过了某个字段"
   - Recommendation：`trxids` 应视为独立字段参与 AND：若配置了 `usernames` 且预扫描注入了 `trxids`，则记录必须同时满足两者

---

## Environment Availability

Step 2.6: SKIPPED（本阶段为纯 Rust 代码变更，无外部服务/CLI 依赖；`regex` crate 通过 `cargo add` 安装，构建环境已具备 cargo）

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `cargo test` |
| Config file | `Cargo.toml` (harness = true, 默认) |
| Quick run command | `cargo test -p dm-database-sqllog2db filters` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FILTER-01 | 配置了正则的字段只保留匹配记录 | unit | `cargo test -p dm-database-sqllog2db filters::tests` | ✅ (filters.rs 有 tests 模块，需新增用例) |
| FILTER-01 | 无效正则在 validate() 时返回 ConfigError | unit | `cargo test -p dm-database-sqllog2db config::tests` | ✅ (config.rs 有 tests 模块，需新增用例) |
| FILTER-02 | 多字段同时配置时，所有字段都必须匹配才保留 | unit | `cargo test -p dm-database-sqllog2db filters::tests` | ✅ (需新增 AND 语义测试用例) |
| FILTER-02 | 单字段配置时行为与旧版相同 | unit | `cargo test -p dm-database-sqllog2db filters::tests` | ✅ (现有用例覆盖，需验证新实现未破坏) |
| FILTER-01/02 | 未配置任何过滤时 pipeline.is_empty() 快路径有效 | unit | `cargo test -p dm-database-sqllog2db` | ✅ (`test_has_filters_empty` 等现有用例) |

### Sampling Rate

- **Per task commit:** `cargo test -p dm-database-sqllog2db filters`
- **Per wave merge:** `cargo test && cargo clippy --all-targets -- -D warnings`
- **Phase gate:** `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

### Wave 0 Gaps

需要在现有 `filters.rs tests` 模块中新增以下测试（文件已存在，仅添加用例）：

- [ ] `test_and_semantics_both_fields_required` — 同时配置 `usernames` 和 `client_ips`，只有两者都满足的记录通过
- [ ] `test_regex_pattern_match` — 正则 `^admin.*` 匹配 `admin_dba`，不匹配 `sys_admin`
- [ ] `test_invalid_regex_returns_error` — 在 `config.rs` tests 中验证非法正则触发 `ConfigError::InvalidValue`
- [ ] `test_record_sql_regex_include` — `record_sql.include_patterns` 正则匹配
- [ ] `test_record_sql_regex_exclude` — `record_sql.exclude_patterns` 正则匹配

---

## Security Domain

本阶段无网络接口、无用户认证、无数据存储写入。`regex` crate 使用有限自动机（非回溯引擎），**不存在 ReDoS（正则拒绝服务）风险**，即使用户配置了复杂正则也能线性时间返回。[VERIFIED: regex crate 官方文档 — "guarantees linear time matching on all inputs"]

ASVS 分析：不适用（CLI 工具，无 web/API 层）。

---

## Sources

### Primary (HIGH confidence)
- `cargo info regex` + `cargo search regex` — 确认版本 1.12.3 [VERIFIED]
- Context7 `/rust-lang/regex` — `RegexSet`、`Regex::new`、`is_match` API 文档 [VERIFIED]
- `src/features/filters.rs` — 现有 `MetaFilters`、`SqlFilters`、`should_keep()` 实现 [VERIFIED]
- `src/config.rs` — `Config::validate()`、`ConfigError::InvalidValue` 模式 [VERIFIED]
- `src/cli/run.rs` — `FilterProcessor`、`process_with_meta` 热路径 [VERIFIED]
- `src/error.rs` — `Error`、`ConfigError` 枚举定义 [VERIFIED]
- `Cargo.toml` — 确认 `regex` 依赖未引入 [VERIFIED]

### Secondary (MEDIUM confidence)
- 无

### Tertiary (LOW confidence)
- 无

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — regex 1.12.3 通过 cargo 确认
- Architecture: HIGH — 基于对现有代码的完整阅读
- Pitfalls: HIGH — 基于代码中现有实现的直接分析

**Research date:** 2026-04-18
**Valid until:** 2026-05-18（regex crate 稳定，30 天内不会有破坏性变更）

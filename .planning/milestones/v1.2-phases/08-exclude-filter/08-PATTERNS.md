# Phase 8: 排除过滤器 - Pattern Map

**Mapped:** 2026-05-10
**Files analyzed:** 3 (需新增/修改的文件)
**Analogs found:** 3 / 3

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/features/filters.rs` | filter/model | request-response (热路径记录级过滤) | `src/features/filters.rs` 内部现有 include 字段模式 | exact — 扩展同文件 |
| `src/cli/run.rs` | orchestration/processor | request-response (pipeline 热路径) | `src/cli/run.rs` 内部现有 `FilterProcessor` | exact — 扩展同文件 |
| `src/cli/init.rs` | config-template | — | `src/cli/init.rs` 内部现有 include 注释块 | exact — 扩展同文件 |

---

## Pattern Assignments

### `src/features/filters.rs` — MetaFilters struct 扩展

**Analog:** 同文件 L61–74（现有 include 字段定义）

**新增 exclude 字段的模式** (L61–74，复制结构，加 `exclude_` 前缀):
```rust
// 现有 include 字段（供对照）
pub struct MetaFilters {
    pub sess_ids: Option<Vec<String>>,
    pub thrd_ids: Option<Vec<String>>,
    pub usernames: Option<Vec<String>>,
    pub statements: Option<Vec<String>>,
    pub appnames: Option<Vec<String>>,
    pub client_ips: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    // ... trxids, start_ts, end_ts
}

// 新增 exclude 字段，紧跟对应 include 字段后：
pub exclude_usernames: Option<Vec<String>>,
pub exclude_client_ips: Option<Vec<String>>,
pub exclude_sess_ids: Option<Vec<String>>,
pub exclude_thrd_ids: Option<Vec<String>>,
pub exclude_statements: Option<Vec<String>>,
pub exclude_appnames: Option<Vec<String>>,
pub exclude_tags: Option<Vec<String>>,
```
serde 通过 `#[serde(flatten)]` on `FiltersFeature.meta` (L47–48) 自动处理 TOML 平铺，无需额外注解。

---

### `src/features/filters.rs` — MetaFilters::has_filters() 扩展

**Analog:** L201–212（现有 has_filters，OR 链检查 include 字段）

**现有模式** (L202–212):
```rust
pub fn has_filters(&self) -> bool {
    self.trxids.as_ref().is_some_and(|v| !v.is_empty())
        || self.client_ips.as_ref().is_some_and(|v| !v.is_empty())
        || self.sess_ids.as_ref().is_some_and(|v| !v.is_empty())
        || self.thrd_ids.as_ref().is_some_and(|v| !v.is_empty())
        || self.usernames.as_ref().is_some_and(|v| !v.is_empty())
        || self.statements.as_ref().is_some_and(|v| !v.is_empty())
        || self.appnames.as_ref().is_some_and(|v| !v.is_empty())
        || self.tags.as_ref().is_some_and(|v| !v.is_empty())
}
```

**扩展方式：** 在末尾追加 7 个 `|| self.exclude_*.as_ref().is_some_and(|v| !v.is_empty())` 分支，与 include 字段完全对称。决策 D-06：任一 exclude 字段非空即返回 `true`，确保纯 exclude 配置也激活 pipeline。

---

### `src/features/filters.rs` — CompiledMetaFilters struct 扩展

**Analog:** L294–305（现有 include 字段的编译后结构）

**现有模式** (L296–305):
```rust
pub struct CompiledMetaFilters {
    pub usernames: Option<Vec<Regex>>,
    pub client_ips: Option<Vec<Regex>>,
    pub sess_ids: Option<Vec<Regex>>,
    pub thrd_ids: Option<Vec<Regex>>,
    pub statements: Option<Vec<Regex>>,
    pub appnames: Option<Vec<Regex>>,
    pub tags: Option<Vec<Regex>>,
    pub trxids: Option<TrxidSet>,
}
```

**扩展方式：** 在结构体末尾追加 7 个 exclude 字段，类型相同：
```rust
pub exclude_usernames: Option<Vec<Regex>>,
pub exclude_client_ips: Option<Vec<Regex>>,
pub exclude_sess_ids: Option<Vec<Regex>>,
pub exclude_thrd_ids: Option<Vec<Regex>>,
pub exclude_statements: Option<Vec<Regex>>,
pub exclude_appnames: Option<Vec<Regex>>,
pub exclude_tags: Option<Vec<Regex>>,
```

---

### `src/features/filters.rs` — CompiledMetaFilters::from_meta() 扩展

**Analog:** L314–326（现有 include 字段的编译构造函数）

**现有模式** (L316–325):
```rust
pub fn from_meta(meta: &MetaFilters) -> Self {
    Self {
        usernames: compile_patterns(meta.usernames.as_deref()).expect("regex validated"),
        client_ips: compile_patterns(meta.client_ips.as_deref()).expect("regex validated"),
        sess_ids: compile_patterns(meta.sess_ids.as_deref()).expect("regex validated"),
        thrd_ids: compile_patterns(meta.thrd_ids.as_deref()).expect("regex validated"),
        statements: compile_patterns(meta.statements.as_deref()).expect("regex validated"),
        appnames: compile_patterns(meta.appnames.as_deref()).expect("regex validated"),
        tags: compile_patterns(meta.tags.as_deref()).expect("regex validated"),
        trxids: meta.trxids.clone(),
    }
}
```

**扩展方式：** 在 `Self { ... }` 块末尾追加 7 个 exclude 字段，完全复用 `compile_patterns` 函数（L253–266），pattern 完全对称：
```rust
exclude_usernames: compile_patterns(meta.exclude_usernames.as_deref()).expect("regex validated"),
exclude_client_ips: compile_patterns(meta.exclude_client_ips.as_deref()).expect("regex validated"),
// ... 余下 5 个字段，模式相同
```

---

### `src/features/filters.rs` — CompiledMetaFilters::has_filters() 扩展（新增 has_any_filters）

**Analog:** L329–339（现有 has_filters，仅检查 include 字段）

**现有模式** (L330–339):
```rust
pub fn has_filters(&self) -> bool {
    self.usernames.is_some()
        || self.client_ips.is_some()
        || self.sess_ids.is_some()
        || self.thrd_ids.is_some()
        || self.statements.is_some()
        || self.appnames.is_some()
        || self.tags.is_some()
        || self.trxids.as_ref().is_some_and(|v| !v.is_empty())
}
```

**扩展方式（决策 D-05）：** 新增 `has_any_filters()` 方法（或重命名现有 `has_filters` 为 `has_include_filters` 后新增），追加 7 个 exclude 字段的 `is_some()` 检查：
```rust
pub fn has_any_filters(&self) -> bool {
    self.has_filters()  // 现有 include 检查
        || self.exclude_usernames.is_some()
        || self.exclude_client_ips.is_some()
        || self.exclude_sess_ids.is_some()
        || self.exclude_thrd_ids.is_some()
        || self.exclude_statements.is_some()
        || self.exclude_appnames.is_some()
        || self.exclude_tags.is_some()
}
```

---

### `src/features/filters.rs` — CompiledMetaFilters::should_keep() 重构

**Analog 1（exclude 语义）:** L413–432，`CompiledSqlFilters::matches()` — exclude 短路模式
```rust
// exclude 先检查（任一命中 → 立即返回 false）
if let Some(excl) = &self.exclude_patterns {
    if excl.iter().any(|re| re.is_match(sql)) {
        return false;
    }
}
```

**Analog 2（现有 include AND 检查）:** L344–379，现有 `should_keep()` 热路径
```rust
#[inline]
#[must_use]
pub fn should_keep(&self, meta: &RecordMeta) -> bool {
    if !match_any_regex(self.usernames.as_deref(), meta.user) {
        return false;
    }
    if !match_any_regex(self.client_ips.as_deref(), meta.ip) {
        return false;
    }
    // ... 其余字段
    true
}
```

**重构方式（决策 D-04）：** 在现有 include 检查之前，插入 exclude OR-veto 短路块：
```rust
pub fn should_keep(&self, meta: &RecordMeta) -> bool {
    // === 1. Exclude OR-veto（任一命中 → 丢弃，短路最快）===
    // match_any_regex(None, _) = true（未配置 = 通过），故取反后：
    // 命中 exclude 时 match_any_regex 返回 true → !true = false → return false
    if self.exclude_usernames.is_some()
        && match_any_regex(self.exclude_usernames.as_deref(), meta.user)
    {
        return false;
    }
    if self.exclude_client_ips.is_some()
        && match_any_regex(self.exclude_client_ips.as_deref(), meta.ip)
    {
        return false;
    }
    // ... 余下 5 个 exclude 字段，tags 需要与 Option<&str> 特殊处理（见下方）

    // === 2. Include AND 检查（现有逻辑不变）===
    if !match_any_regex(self.usernames.as_deref(), meta.user) {
        return false;
    }
    // ... 现有代码继续
    true
}
```

**exclude_tags 特殊处理：** 参照现有 include tags 处理（L371–378），`meta.tag` 为 `Option<&str>`：
```rust
// exclude: 有 tag 值且命中 exclude 规则 → 丢弃；无 tag 值时不触发 exclude
if let (Some(excl_tags), Some(t)) = (&self.exclude_tags, meta.tag) {
    if excl_tags.iter().any(|re| re.is_match(t)) {
        return false;
    }
}
```

---

### `src/features/filters.rs` — FiltersFeature::validate_regexes() 扩展

**Analog:** L106–129（现有 validate_regexes，追加 validate_pattern_list 调用模式）

**现有模式** (L107–119，7 个 include 字段校验):
```rust
pub fn validate_regexes(&self) -> crate::error::Result<()> {
    validate_pattern_list("features.filters.usernames", self.meta.usernames.as_deref())?;
    validate_pattern_list("features.filters.client_ips", self.meta.client_ips.as_deref())?;
    validate_pattern_list("features.filters.sess_ids", self.meta.sess_ids.as_deref())?;
    validate_pattern_list("features.filters.thrd_ids", self.meta.thrd_ids.as_deref())?;
    validate_pattern_list("features.filters.statements", self.meta.statements.as_deref())?;
    validate_pattern_list("features.filters.appnames", self.meta.appnames.as_deref())?;
    validate_pattern_list("features.filters.tags", self.meta.tags.as_deref())?;
    // ...record_sql 的两个调用
    Ok(())
}
```

**扩展方式（决策 D-08）：** 在现有 7 个 include 校验之后、record_sql 之前，追加 7 个 exclude 校验：
```rust
validate_pattern_list("features.filters.exclude_usernames", self.meta.exclude_usernames.as_deref())?;
validate_pattern_list("features.filters.exclude_client_ips", self.meta.exclude_client_ips.as_deref())?;
validate_pattern_list("features.filters.exclude_sess_ids", self.meta.exclude_sess_ids.as_deref())?;
validate_pattern_list("features.filters.exclude_thrd_ids", self.meta.exclude_thrd_ids.as_deref())?;
validate_pattern_list("features.filters.exclude_statements", self.meta.exclude_statements.as_deref())?;
validate_pattern_list("features.filters.exclude_appnames", self.meta.exclude_appnames.as_deref())?;
validate_pattern_list("features.filters.exclude_tags", self.meta.exclude_tags.as_deref())?;
```
`validate_pattern_list` 函数（L278–292）直接复用，无需修改。

---

### `src/cli/run.rs` — FilterProcessor::new() 扩展

**Analog:** L44–55（现有 new()，预计算 has_meta_filters）

**现有模式** (L45–54):
```rust
fn new(filter: &crate::features::FiltersFeature) -> Self {
    let compiled_meta = CompiledMetaFilters::from_meta(&filter.meta);
    let has_meta_filters = compiled_meta.has_filters();
    Self {
        compiled_meta,
        start_ts: filter.meta.start_ts.clone(),
        end_ts: filter.meta.end_ts.clone(),
        has_meta_filters,
    }
}
```

**扩展方式（决策 D-05）：** `has_meta_filters` 改为调用 `has_any_filters()`（或等价方法），确保纯 exclude 配置也激活 meta 检查路径：
```rust
let has_meta_filters = compiled_meta.has_any_filters(); // 包含 exclude 字段
```
其余代码无需修改。

---

### `src/cli/init.rs` — verbose 模板（中文 CONFIG_TEMPLATE_ZH）

**Analog:** L88–106（现有 meta filters 注释块，每个 include 字段一行注释）

**现有模式** (L92–105，以 usernames 为例):
```toml
# 过滤指定的用户名（支持模糊匹配）
# usernames = ["SYSDBA"]
```

**扩展方式（决策 D-09）：** 在每个 include 注释行下方紧跟对应 exclude 注释，格式对称：
```toml
# 过滤指定的用户名（支持正则匹配）
# usernames = ["SYSDBA"]
# 排除指定的用户名（OR veto：任一命中则丢弃该记录）
# exclude_usernames = ["guest", "^anon"]
```
需对 7 个字段（client_ips / usernames / sess_ids / thrd_ids / statements / appnames / tags）各插入一对注释行。tags 字段当前 ZH 模板未显示，按 EN 模板同步补充。

---

### `src/cli/init.rs` — minimal 模板（英文 CONFIG_TEMPLATE_EN）

**Analog:** L168–185（现有 meta filters 注释块，英文版）

**现有模式** (L172–185，以 usernames 为例):
```toml
# Filter by usernames (substring match)
# usernames = ["SYSDBA"]
```

**扩展方式（决策 D-09）：** 在每个 include 注释行下方插入 exclude 对应行：
```toml
# Filter by usernames (regex match)
# usernames = ["SYSDBA"]
# Exclude by usernames (OR veto: any match drops the record)
# exclude_usernames = ["guest", "^anon"]
```
同样对 7 个字段处理。注意现有注释说"substring match"，实际已是正则，注释应同步修正为"regex match"（此改动在 D-09 范围内）。

---

## Shared Patterns（跨文件共用）

### compile_patterns — 正则编译函数
**Source:** `src/features/filters.rs` L253–266
**Apply to:** `CompiledMetaFilters::from_meta()` 中 7 个 exclude 字段的编译
```rust
fn compile_patterns(
    patterns: Option<&[String]>,
) -> std::result::Result<Option<Vec<Regex>>, String> {
    match patterns {
        None | Some([]) => Ok(None),
        Some(v) => {
            let compiled = v
                .iter()
                .map(|p| Regex::new(p).map_err(|_| p.clone()))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(Some(compiled))
        }
    }
}
```
直接复用，无需修改。

### match_any_regex — 热路径正则匹配
**Source:** `src/features/filters.rs` L269–275
**Apply to:** `CompiledMetaFilters::should_keep()` 中 exclude 字段的匹配（命中返回 `true`，取反即为排除）
```rust
#[inline]
fn match_any_regex(patterns: Option<&[Regex]>, val: &str) -> bool {
    match patterns {
        None | Some([]) => true,
        Some(p) => p.iter().any(|re| re.is_match(val)),
    }
}
```
exclude 用法：`match_any_regex(self.exclude_usernames.as_deref(), meta.user)` 返回 `true` 表示命中 exclude 规则，应丢弃记录（`return false`）。

### validate_pattern_list — 正则格式校验
**Source:** `src/features/filters.rs` L278–292
**Apply to:** `FiltersFeature::validate_regexes()` 追加的 7 个 exclude 字段校验
```rust
fn validate_pattern_list(field: &str, patterns: Option<&[String]>) -> crate::error::Result<()> {
    let Some(list) = patterns else {
        return Ok(());
    };
    for pattern in list {
        Regex::new(pattern).map_err(|e| {
            crate::error::Error::Config(crate::error::ConfigError::InvalidValue {
                field: field.to_string(),
                value: pattern.clone(),
                reason: format!("invalid regex: {e}"),
            })
        })?;
    }
    Ok(())
}
```
直接复用，无需修改。

### has_filters OR 链模式
**Source:** `src/features/filters.rs` L201–212 (`MetaFilters::has_filters`)
**Apply to:** 新 exclude 字段加入同一 OR 链（决策 D-06），及 `CompiledMetaFilters::has_any_filters()`
```rust
// 模式：.as_ref().is_some_and(|v| !v.is_empty())
self.exclude_usernames.as_ref().is_some_and(|v| !v.is_empty())
    || self.exclude_client_ips.as_ref().is_some_and(|v| !v.is_empty())
    // ...
```

### CompiledSqlFilters exclude 短路模式（参考 exclude 语义实现）
**Source:** `src/features/filters.rs` L418–432 (`CompiledSqlFilters::matches`)
```rust
// exclude 先于 include，任一命中立即返回 false
if let Some(excl) = &self.exclude_patterns {
    if excl.iter().any(|re| re.is_match(sql)) {
        return false;
    }
}
```
`CompiledMetaFilters::should_keep()` 的 exclude 块遵循相同短路语义，但改用 `match_any_regex` 保持一致性。

---

## No Analog Found

无——所有需修改的文件和模式在现有代码中均有精确对应的参照实现。

---

## Metadata

**Analog search scope:** `src/features/`, `src/cli/`
**Files scanned:** 3 (`filters.rs`, `run.rs`, `init.rs`)
**Pattern extraction date:** 2026-05-10

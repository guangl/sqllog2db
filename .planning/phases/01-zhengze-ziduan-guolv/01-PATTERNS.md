# Phase 1: 正则字段过滤 - Pattern Map

**Mapped:** 2026-04-18
**Files analyzed:** 3 (需修改的文件)
**Analogs found:** 3 / 3

---

## File Classification

| 需修改文件 | Role | Data Flow | Closest Analog | Match Quality |
|-----------|------|-----------|----------------|---------------|
| `src/features/filters.rs` | service / filter | request-response (热路径逐条判断) | `src/features/filters.rs` 自身（MetaFilters、SqlFilters 现有实现） | exact — 在现有结构上外科改造 |
| `src/config.rs` | config / validator | batch (启动阶段单次验证) | `src/config.rs` 现有 `Config::validate()` + `LoggingConfig::validate()` | exact — 沿用相同 validate 模式 |
| `src/cli/run.rs` | orchestrator / hot-path | streaming | `src/cli/run.rs` 现有 `FilterProcessor::new()` + `process_log_file()` | exact — 仅修改 FilterProcessor 内部字段 |

---

## Pattern Assignments

### `src/features/filters.rs`（主要改动文件）

**Analog:** `src/features/filters.rs`（自身现有实现）

---

#### 1. 新增 `CompiledMetaFilters` 结构

**参考：** 现有 `MetaFilters` 结构（第 60-73 行）和 `vec_to_hashset` 反序列化函数（第 22-30 行）

**现有 MetaFilters 声明模式**（lines 60-73）：
```rust
#[derive(Debug, Deserialize, Clone, Default)]
pub struct MetaFilters {
    pub start_ts: Option<String>,
    pub end_ts: Option<String>,
    pub sess_ids: Option<Vec<String>>,
    pub thrd_ids: Option<Vec<String>>,
    pub usernames: Option<Vec<String>>,
    #[serde(default, deserialize_with = "vec_to_hashset")]
    pub trxids: Option<TrxidSet>,
    pub statements: Option<Vec<String>>,
    pub appnames: Option<Vec<String>>,
    pub client_ips: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}
```

**新增 CompiledMetaFilters 应遵循的模式：**
- `MetaFilters` 保持不变（serde Deserialize，存原始字符串）
- 新增 `CompiledMetaFilters` 结构，字段从 `Option<Vec<String>>` 改为 `Option<Vec<regex::Regex>>`
- `trxids` 保持 `Option<TrxidSet>`（精确匹配，不升级为正则）
- 新增模块级私有辅助函数 `compile_patterns()` 和 `match_any_regex()`

---

#### 2. `should_keep()` 跨字段 AND 改造

**现有 OR 逻辑**（`MetaFilters::should_keep`，lines 169-182）：
```rust
pub fn should_keep(&self, meta: &RecordMeta) -> bool {
    // OR 逻辑：命中任何一个已定义的列表即保留
    Self::match_exact(self.trxids.as_ref(), meta.trxid)
        || Self::match_substring(self.client_ips.as_ref(), meta.ip)
        || Self::match_substring(self.sess_ids.as_ref(), meta.sess)
        || Self::match_substring(self.thrd_ids.as_ref(), meta.thrd)
        || Self::match_substring(self.usernames.as_ref(), meta.user)
        || Self::match_substring(self.statements.as_ref(), meta.stmt)
        || Self::match_substring(self.appnames.as_ref(), meta.app)
        || meta
            .tag
            .is_some_and(|t| Self::match_substring(self.tags.as_ref(), t))
}
```

**改为 AND 逻辑后应遵循的模式（`CompiledMetaFilters::should_keep`）：**
```rust
#[inline]
#[must_use]
pub fn should_keep(&self, meta: &RecordMeta) -> bool {
    // AND 语义：每个配置了过滤条件的字段都必须通过
    if !match_any_regex(self.usernames.as_deref(), meta.user)   { return false; }
    if !match_any_regex(self.client_ips.as_deref(), meta.ip)    { return false; }
    if !match_any_regex(self.sess_ids.as_deref(), meta.sess)    { return false; }
    if !match_any_regex(self.thrd_ids.as_deref(), meta.thrd)    { return false; }
    if !match_any_regex(self.statements.as_deref(), meta.stmt)  { return false; }
    if !match_any_regex(self.appnames.as_deref(), meta.app)     { return false; }
    // trxids：精确匹配，不变
    if let Some(trxids) = &self.trxids {
        if !trxids.is_empty() && !trxids.contains(meta.trxid) { return false; }
    }
    // tags：记录的 tag 字段为 Option<&str>，需特殊处理
    if let Some(tag_patterns) = &self.tags {
        match meta.tag {
            Some(t) if !tag_patterns.iter().any(|re| re.is_match(t)) => return false,
            None if !tag_patterns.is_empty() => return false,
            _ => {}
        }
    }
    true
}
```

**注意：** `tags` 的特殊处理不能套用 `match_any_regex()` 辅助，因为 `meta.tag` 是 `Option<&str>` 而非 `&str`。

---

#### 3. 辅助函数模式

**现有辅助函数模式**（lines 184-195，用于参考结构风格）：
```rust
/// O(1) 精确匹配，适用于高基数的 trxid 集合。
fn match_exact(set: Option<&TrxidSet>, val: &str) -> bool {
    set.is_some_and(|s| !s.is_empty() && s.contains(val))
}

/// O(n) 子串匹配，适用于小型过滤列表
fn match_substring(list: Option<&Vec<String>>, val: &str) -> bool {
    list.is_some_and(|items| {
        !items.is_empty() && items.iter().any(|i| val.contains(i.as_str()))
    })
}
```

**新增辅助函数应遵循同一私有函数风格（无 pub，文件级 fn）：**
```rust
/// 编译一组正则字符串。空列表或 None 均转为 None（表示"不参与过滤"）。
/// 任一 pattern 非法时返回失败的 pattern 字符串，供调用方构造 ConfigError。
fn compile_patterns(
    patterns: Option<&Vec<String>>,
) -> Result<Option<Vec<regex::Regex>>, String> {
    match patterns {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(v) => {
            let compiled = v
                .iter()
                .map(|p| regex::Regex::new(p).map_err(|_| p.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(compiled))
        }
    }
}

/// None 表示"未配置，通过"；Some([]) 同理；Some(patterns) 需任意一个命中。
#[inline]
fn match_any_regex(patterns: Option<&[regex::Regex]>, val: &str) -> bool {
    match patterns {
        None => true,
        Some(p) if p.is_empty() => true,
        Some(p) => p.iter().any(|re| re.is_match(val)),
    }
}
```

---

#### 4. `CompiledSqlFilters` 结构（记录级正则）

**现有 `SqlFilters::matches()` 模式**（lines 244-273）：
```rust
pub fn matches(&self, sql: &str) -> bool {
    if !self.has_filters() {
        return false;
    }
    // include：命中其中之一
    let include_match = if let Some(patterns) = &self.include_patterns {
        if patterns.is_empty() { true } else { patterns.iter().any(|p| sql.contains(p)) }
    } else {
        true
    };
    if !include_match { return false; }
    // exclude：不能命中任何一个
    if let Some(patterns) = &self.exclude_patterns {
        if patterns.iter().any(|p| sql.contains(p)) { return false; }
    }
    true
}
```

**新增 `CompiledSqlFilters::matches()` 保持相同结构，将 `sql.contains(p)` 替换为 `re.is_match(sql)`：**
- `SqlFilters` 原始结构不变（serde 用途 + 事务级预扫描用的 `sql` 字段继续使用字符串包含）
- 新增 `CompiledSqlFilters { include_patterns: Option<Vec<Regex>>, exclude_patterns: Option<Vec<Regex>> }`
- `#[must_use]` 标注保持一致

---

#### 5. 测试模式

**现有测试辅助函数风格**（lines 279-363）：
```rust
fn make_feature(enable: bool) -> FiltersFeature {
    FiltersFeature {
        enable,
        meta: MetaFilters::default(),
        indicators: IndicatorFilters::default(),
        sql: SqlFilters::default(),
        record_sql: SqlFilters::default(),
    }
}

fn m<'a>(trxid: &'a str, ip: &'a str, user: &'a str, tag: Option<&'a str>) -> RecordMeta<'a> {
    RecordMeta { trxid, ip, sess: "s", thrd: "t", user, stmt: "st", app: "a", tag }
}
```

**新增测试用例应在 `#[cfg(test)] mod tests` 内延续此风格：**
- 用 `make_feature()` 构造基础配置，然后设置字段
- 用 `m()` 辅助构造 `RecordMeta`
- 断言用 `assert!` / `assert!(!...)`，不要 `assert_eq!(bool, true)`

---

### `src/config.rs`（新增正则验证）

**Analog:** `src/config.rs` 现有 `Config::validate()` 和 `LoggingConfig::validate()` 模式

---

#### 现有 validate() 扩展点（lines 54-73）：
```rust
pub fn validate(&self) -> Result<()> {
    self.logging.validate()?;
    self.exporter.validate()?;
    self.sqllog.validate()?;
    if let Some(names) = &self.features.fields {
        for name in names {
            if !crate::features::FIELD_NAMES.contains(&name.as_str()) {
                return Err(Error::Config(ConfigError::InvalidValue {
                    field: "features.fields".to_string(),
                    value: name.clone(),
                    reason: format!("unknown field '{name}'; ..."),
                }));
            }
        }
    }
    Ok(())
}
```

**新增正则验证应在 `self.sqllog.validate()?;` 之后添加：**
```rust
if let Some(filters) = &self.features.filters {
    if filters.enable {
        filters.validate_regexes()?;
    }
}
```

---

#### ConfigError::InvalidValue 模式（lines 55-59 in error.rs）：
```rust
#[error("Invalid configuration value {field} = '{value}': {reason}")]
InvalidValue {
    field: String,
    value: String,
    reason: String,
}
```

**`validate_pattern_list()` 辅助函数应在 `src/features/filters.rs` 或 `FiltersFeature::validate_regexes()` 中实现，返回此类型错误：**
```rust
fn validate_pattern_list(field: &str, patterns: Option<&Vec<String>>) -> crate::error::Result<()> {
    let Some(list) = patterns else { return Ok(()); };
    for pattern in list {
        regex::Regex::new(pattern).map_err(|e| {
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

---

#### 现有测试中 validate 错误断言风格（lines 385-461）：
```rust
#[test]
fn test_validate_empty_logging_file() {
    let mut cfg = default_config();
    cfg.logging.file = "  ".into();
    assert!(cfg.validate().is_err());
}
```

**新增正则验证测试应在 `config.rs` 的 `tests` 模块中，继续此风格。**

---

### `src/cli/run.rs`（FilterProcessor 改造）

**Analog:** `src/cli/run.rs` 现有 `FilterProcessor` struct 和 `impl FilterProcessor::new()` 模式

---

#### 现有 FilterProcessor 结构（lines 33-49）：
```rust
#[derive(Debug)]
struct FilterProcessor {
    filter: crate::features::FiltersFeature,
    /// 预计算：filter.meta.has_filters() 的结果
    has_meta_filters: bool,
}

impl FilterProcessor {
    fn new(filter: crate::features::FiltersFeature) -> Self {
        let has_meta_filters = filter.meta.has_filters();
        Self { filter, has_meta_filters }
    }
}
```

**修改后结构：将 `filter: FiltersFeature`（含原始字符串）替换为预编译结构：**
```rust
#[derive(Debug)]
struct FilterProcessor {
    // 时间范围直接存字符串（比较用，不需要正则）
    start_ts: Option<String>,
    end_ts:   Option<String>,
    // 预编译的元数据过滤器（热路径使用）
    compiled_meta: crate::features::filters::CompiledMetaFilters,
    // 预编译的记录级 SQL 过滤器
    compiled_record_sql: crate::features::filters::CompiledSqlFilters,
    /// 预计算：compiled_meta 是否有任何非 None 字段
    has_meta_filters: bool,
}
```

**`has_meta_filters` 的计算必须基于编译后的 `CompiledMetaFilters`，不能继续调用旧的 `filter.meta.has_filters()`（见 RESEARCH.md Pitfall 2）。**

---

#### process_with_meta 热路径模式（lines 61-95）：
```rust
fn process_with_meta(
    &self,
    record: &dm_database_parser_sqllog::Sqllog,
    meta: &MetaParts<'_>,
) -> bool {
    let ts = record.ts.as_ref();

    // 时间过滤：无需构造 RecordMeta
    if let Some(start) = &self.filter.meta.start_ts {
        if ts < start.as_str() && !ts.starts_with(start.as_str()) {
            return false;
        }
    }
    // ...
    // 快速路径：无元数据过滤 → 直接通过
    if !self.has_meta_filters {
        return true;
    }

    self.filter.meta.should_keep(&RecordMeta { ... })
}
```

**修改后，`self.filter.meta.start_ts` 改为 `self.start_ts`，`self.filter.meta.should_keep()` 改为 `self.compiled_meta.should_keep()`，结构保持不变。**

---

#### sql_record_filter 在 handle_run 中的使用模式（lines 647-653）：
```rust
let sql_record_filter: Option<&SqlFilters> = final_cfg
    .features
    .filters
    .as_ref()
    .filter(|f| f.enable && f.record_sql.has_filters())
    .map(|f| &f.record_sql);
```

**修改后，`sql_record_filter` 的类型需改为 `Option<&CompiledSqlFilters>`，在 `build_pipeline` 或 `handle_run` 内预编译后传递。**

---

## Shared Patterns（跨文件公共模式）

### 错误处理模式
**来源：** `src/error.rs`（lines 41-63）+ `src/config.rs`（lines 61-71）
**适用于：** `FiltersFeature::validate_regexes()`、`validate_pattern_list()` 函数
```rust
return Err(Error::Config(ConfigError::InvalidValue {
    field: "features.filters.usernames".to_string(),
    value: pattern.clone(),
    reason: format!("invalid regex: {e}"),
}));
```

### `#[inline]` + `#[must_use]` 热路径标注
**来源：** `src/features/filters.rs`（lines 168、205、244）及 `src/features/mod.rs`（lines 54、179）
**适用于：** `CompiledMetaFilters::should_keep()`、`match_any_regex()`、`CompiledSqlFilters::matches()`
```rust
#[inline]
#[must_use]
pub fn should_keep(&self, meta: &RecordMeta) -> bool { ... }
```

### `has_filters()` 快路径检查模式
**来源：** `src/features/filters.rs`（lines 157-166）—— `MetaFilters::has_filters()`
```rust
pub fn has_filters(&self) -> bool {
    self.trxids.as_ref().is_some_and(|v| !v.is_empty())
        || self.client_ips.as_ref().is_some_and(|v| !v.is_empty())
        || ...
}
```
**`CompiledMetaFilters` 应新增对应的 `has_filters()` 方法，基于 `Option<Vec<Regex>>` 字段是否为 `Some` 来判断。**

### 启动阶段 expect() 而非运行时 unwrap()
**来源：** `src/cli/run.rs`（FilterProcessor::new 构造）
**适用于：** `CompiledMetaFilters::from_config()`、`CompiledSqlFilters::from_config()` 在 `FilterProcessor::new()` 中的调用：
```rust
let compiled_meta = CompiledMetaFilters::from_config(&filter.meta)
    .expect("regexes already validated in Config::validate()");
```

---

## No Analog Found

本阶段所有改动都在已有文件中进行，无需新文件。全部有精确 analog。

| 文件 | 说明 |
|------|------|
| 无 | 所有改动为现有文件的外科手术式修改 |

---

## 关键约束提醒（来自 RESEARCH.md）

| 约束 | 影响文件 |
|------|----------|
| `regex` crate 需新增到 `Cargo.toml` | `Cargo.toml` |
| 事务级 `sql` 字段的 `SqlFilters` 不升级为正则（D-03） | `filters.rs`、`run.rs` |
| `has_meta_filters` 必须基于 `CompiledMetaFilters` 重新计算 | `run.rs` |
| `tags` 字段 `meta.tag` 为 `Option<&str>`，不能套用通用 `match_any_regex` | `filters.rs` |
| `None` → 通过（不参与 AND），`Some([])` → 同 `None` | `filters.rs` |

---

## Metadata

**Analog search scope:** `src/features/`、`src/config.rs`、`src/cli/run.rs`、`src/error.rs`
**Files scanned:** 5
**Pattern extraction date:** 2026-04-18

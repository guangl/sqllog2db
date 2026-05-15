# Phase 9: CLI 启动提速 - Pattern Map

**Mapped:** 2026-05-11
**Files analyzed:** 4（修改文件）+ 1（文档新增节）
**Analogs found:** 4 / 4

## File Classification

| 修改文件 | Role | Data Flow | Closest Analog | Match Quality |
|----------|------|-----------|----------------|---------------|
| `src/features/filters.rs` | utility / domain-logic | transform | 自身（重构内部函数签名） | self-refactor |
| `src/config.rs` | config | request-response | `src/config.rs` `LoggingConfig::validate()` (L256-281) | exact |
| `src/cli/update.rs` | utility | event-driven (fire-and-forget) | `src/main.rs` Ctrl+C handler (L203-208) | partial-match |
| `src/main.rs` | cli / orchestration | request-response | 自身（修改调用位置 L144-146） | self-refactor |
| `benches/BENCHMARKS.md` | docs | — | `benches/BENCHMARKS.md` Phase 4/5 节格式 | exact |

---

## Pattern Assignments

### `src/features/filters.rs` — compile_patterns 签名变更 + validate_regexes 删除

**变更点 1：`compile_patterns` 返回类型改为 `crate::Result<Option<Vec<Regex>>>`**

当前签名（L322-336）：
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

改后目标（复制 `validate_pattern_list` 的错误构造模式，L348-362）：
```rust
fn compile_patterns(
    field: &str,
    patterns: Option<&[String]>,
) -> crate::error::Result<Option<Vec<Regex>>> {
    match patterns {
        None | Some([]) => Ok(None),
        Some(v) => {
            let compiled = v
                .iter()
                .map(|p| {
                    Regex::new(p).map_err(|e| {
                        crate::error::Error::Config(crate::error::ConfigError::InvalidValue {
                            field: field.to_string(),
                            value: p.clone(),
                            reason: format!("invalid regex: {e}"),
                        })
                    })
                })
                .collect::<crate::error::Result<Vec<_>>>()?;
            Ok(Some(compiled))
        }
    }
}
```

**错误类型模式（从 `validate_pattern_list` L348-362 直接复制）：**
```rust
crate::error::Error::Config(crate::error::ConfigError::InvalidValue {
    field: field.to_string(),
    value: pattern.clone(),
    reason: format!("invalid regex: {e}"),
})
```

**变更点 2：`CompiledMetaFilters::from_meta` 改为 `try_from_meta`**

当前实现（L384-416），所有字段调用 `.expect("regex validated")`：
```rust
pub fn from_meta(meta: &MetaFilters) -> Self {
    Self {
        usernames: compile_patterns(meta.usernames.as_deref()).expect("regex validated"),
        // ... 14 个字段，每个都是 .expect(...)
    }
}
```

改后目标（全部改为 `?` 传播，函数签名变为 `Result<Self>`）：
```rust
pub fn try_from_meta(meta: &MetaFilters) -> crate::error::Result<Self> {
    Ok(Self {
        usernames: compile_patterns("features.filters.usernames",
                                   meta.usernames.as_deref())?,
        client_ips: compile_patterns("features.filters.client_ips",
                                     meta.client_ips.as_deref())?,
        // ... 所有字段均使用 ? 传播
    })
}
```

**field 参数字符串规范**（从现有 `validate_include_regexes` L127-142 读取，保持字段名一致）：
- `"features.filters.usernames"`
- `"features.filters.client_ips"`
- `"features.filters.sess_ids"`
- `"features.filters.thrd_ids"`
- `"features.filters.statements"`
- `"features.filters.appnames"`
- `"features.filters.tags"`
- `"features.filters.exclude_usernames"`（exclude 系列同理）
- `"features.filters.record_sql.include_patterns"`
- `"features.filters.record_sql.exclude_patterns"`

**变更点 3：`FiltersFeature::validate_regexes()` 完全删除（L111-174）**

删除整个方法（包括 `validate_include_regexes` 和 `validate_exclude_regexes` 私有辅助方法）。

**变更点 4：`CompiledSqlFilters::from_sql_filters` 改为 `try_from_sql_filters`**

当前（L549-562）：
```rust
pub fn from_sql_filters(sf: &SqlFilters) -> Self {
    Self {
        include_patterns: compile_patterns(sf.include_patterns.as_deref())
            .expect("regex validated"),
        exclude_patterns: compile_patterns(sf.exclude_patterns.as_deref())
            .expect("regex validated"),
    }
}
```

改后目标：
```rust
pub fn try_from_sql_filters(sf: &SqlFilters) -> crate::error::Result<Self> {
    Ok(Self {
        include_patterns: compile_patterns(
            "features.filters.record_sql.include_patterns",
            sf.include_patterns.as_deref())?,
        exclude_patterns: compile_patterns(
            "features.filters.record_sql.exclude_patterns",
            sf.exclude_patterns.as_deref())?,
    })
}
```

**测试更新模式**（L1013 `make_compiled_meta` 辅助函数）：

所有调用 `CompiledMetaFilters::from_meta(&meta)` 的地方须改为 `CompiledMetaFilters::try_from_meta(&meta).expect("test fixture")`，或改为 `.unwrap()`。目前测试中有以下调用位置（已读出）：
- L1012：`make_compiled_meta` 辅助
- L1041：`test_compiled_meta_single_field_or`
- L1053：`test_compiled_meta_tags_none_rejected`
- L1072：`test_compiled_meta_trxids_and`
- L1102：`test_t1_compiled_from_meta_exclude_usernames`
- L1108：`test_t1_compiled_from_meta_exclude_none`
- L1118：`test_t1_has_any_filters_include_only`
- L1128：`test_t1_has_any_filters_exclude_only`
- L1137：`make_compiled_with_exclude` 辅助
- L1227：`test_exclude_tags_drops_matching` 等系列
- `test_exclude_invalid_regex_validate_fails`（L1278）须改为调用 `try_from_meta()` 而非 `validate_regexes()`

---

### `src/config.rs` — Config::validate() 调用链变更

**当前调用链（L54-62）：**
```rust
pub fn validate(&self) -> Result<()> {
    self.logging.validate()?;
    self.exporter.validate()?;
    self.sqllog.validate()?;
    if let Some(filters) = &self.features.filters {
        if filters.enable {
            filters.validate_regexes()?;   // ← 删除此调用
        }
    }
    // ...
    Ok(())
}
```

**改后目标（复制 `if let Some(filters)` 块的模式，将 `validate_regexes` 替换为 `try_from_meta`）：**
```rust
pub fn validate(&self) -> Result<()> {
    self.logging.validate()?;
    self.exporter.validate()?;
    self.sqllog.validate()?;
    if let Some(filters) = &self.features.filters {
        if filters.enable {
            // 用 try_from_meta 验证正则（兼做编译验证，结果丢弃）
            CompiledMetaFilters::try_from_meta(&filters.meta)?;
            CompiledSqlFilters::try_from_sql_filters(&filters.record_sql)?;
        }
    }
    // features.fields 检查保持不变（L63-76）
    Ok(())
}
```

**imports 补充**：validate() 上方需确保 `CompiledMetaFilters` 和 `CompiledSqlFilters` 可访问。当前 `src/config.rs` 通过 `pub use crate::features::FeaturesConfig;`（L4）引入 features。需在 validate() 中直接用全路径 `crate::features::CompiledMetaFilters::try_from_meta(...)` 或在文件顶部 use。参考现有模式（L4）：
```rust
pub use crate::features::FeaturesConfig;
// 新增（或在函数内用全路径）：
use crate::features::{CompiledMetaFilters, CompiledSqlFilters};
```

**error 构造模式（复制 L46-50）：**
```rust
.map_err(|e| {
    Error::Config(ConfigError::ParseFailed {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
})
```

---

### `src/cli/run.rs` — FilterProcessor::new 改用 try_from_meta

**当前 FilterProcessor::new（L44-56）：**
```rust
impl FilterProcessor {
    fn new(filter: &crate::features::FiltersFeature) -> Self {
        let compiled_meta = CompiledMetaFilters::from_meta(&filter.meta);
        let has_meta_filters = compiled_meta.has_any_filters();
        Self {
            compiled_meta,
            start_ts: filter.meta.start_ts.clone(),
            end_ts: filter.meta.end_ts.clone(),
            has_meta_filters,
        }
    }
}
```

**改后目标（改名 try_new，返回 Result，由 build_pipeline 或 handle_run 用 `?` 处理）：**
```rust
impl FilterProcessor {
    fn try_new(filter: &crate::features::FiltersFeature) -> crate::error::Result<Self> {
        let compiled_meta = CompiledMetaFilters::try_from_meta(&filter.meta)?;
        let has_meta_filters = compiled_meta.has_any_filters();
        Ok(Self {
            compiled_meta,
            start_ts: filter.meta.start_ts.clone(),
            end_ts: filter.meta.end_ts.clone(),
            has_meta_filters,
        })
    }
}
```

**build_pipeline 调用方变更（L21-31）：**
```rust
// 当前
fn build_pipeline(cfg: &Config) -> Pipeline {
    let mut pipeline = Pipeline::new();
    if let Some(f) = &cfg.features.filters {
        if f.has_filters() {
            pipeline.add(Box::new(FilterProcessor::new(f)));
        }
    }
    pipeline
}

// 改后（返回 Result）
fn build_pipeline(cfg: &Config) -> crate::error::Result<Pipeline> {
    let mut pipeline = Pipeline::new();
    if let Some(f) = &cfg.features.filters {
        if f.has_filters() {
            pipeline.add(Box::new(FilterProcessor::try_new(f)?));
        }
    }
    Ok(pipeline)
}
```

**`?` 传播模式参考：** 整个 `handle_run` 函数已经是 `-> Result<()>`（L 见文件结构），`?` 可直接用于 `build_pipeline(cfg)?`。

---

### `src/cli/update.rs` — check_for_updates_at_startup 后台化

**当前同步实现（L66-91）：**
```rust
pub fn check_for_updates_at_startup() {
    let current_version = cargo_crate_version!();
    let status = self_update::backends::github::Update::configure()
        // ...
        .build();
    if let Ok(status) = status {
        if let Ok(release) = status.get_latest_release() {
            if self_update::version::bump_is_greater(current_version, &release.version)
                .unwrap_or(false)
            {
                warn!("A new version is available: {} ...", release.version, current_version);
                warn!("Run 'sqllog2db self-update' to update.");
            }
        }
    }
}
```

**改后目标（fire-and-forget，参考 `src/main.rs` L203-208 的 thread::spawn 模式）：**
```rust
pub fn check_for_updates_at_startup() {
    std::thread::spawn(|| {
        let current_version = cargo_crate_version!();
        let status = self_update::backends::github::Update::configure()
            .repo_owner("guangl")
            .repo_name("sqllog2db")
            .bin_name("sqllog2db")
            .current_version(current_version)
            .build();
        if let Ok(status) = status {
            if let Ok(release) = status.get_latest_release() {
                if self_update::version::bump_is_greater(current_version, &release.version)
                    .unwrap_or(false)
                {
                    warn!(
                        "A new version is available: {} (current: {})",
                        release.version, current_version
                    );
                    warn!("Run 'sqllog2db self-update' to update.");
                }
            }
        }
    });
    // 不等待 JoinHandle，fire-and-forget
}
```

**thread::spawn 模式出处（`src/main.rs` L203-208）：**
```rust
let interrupted_flag = Arc::clone(&interrupted);
ctrlc::set_handler(move || {
    interrupted_flag.store(true, Ordering::Relaxed);
})
.ok();
```
（这里是 `ctrlc::set_handler`，但 `std::thread::spawn(|| { ... })` 与此同为后台执行，省去 JoinHandle 即 fire-and-forget 语义。）

**`main.rs` 调用位置（L139-146）保持不变**，无需改动：
```rust
if !cli.quiet
    && !matches!(
        &cli.command,
        Some(cli::opts::Commands::SelfUpdate { .. } | cli::opts::Commands::Completions { .. })
    )
{
    cli::update::check_for_updates_at_startup();  // 调用签名不变
}
```

---

### `benches/BENCHMARKS.md` — Phase 9 冷启动基线节（新增）

**新增节格式**（复制 Phase 4/5 节格式，L128-199 / L204-282）：

```markdown
---

## Phase 9 — CLI 冷启动基线（PERF-11）

**Date:** 2026-05-11
**Goal:** 量化双重 regex 编译消除前后的冷启动耗时；记录 hyperfine 原始输出
**Test environment:** Apple Silicon (Darwin 25.4.0), release build

### 测量命令

```bash
hyperfine --warmup 3 'sqllog2db --version'
hyperfine --warmup 3 'sqllog2db validate -c config.toml'
hyperfine --warmup 3 'sqllog2db validate -c config_no_regex.toml'
```

### 对比维度

| 命令 | 优化前 (mean) | 优化后 (mean) | 差值 |
|------|--------------|--------------|------|
| `sqllog2db --version` | — ms | — ms | — |
| `validate`（含 regex） | — ms | — ms | — |
| `validate`（无 regex） | — ms | — ms | — |

### Hyperfine 原始输出

<details>
<summary>优化前</summary>

```
[待填充]
```

</details>

<details>
<summary>优化后</summary>

```
[待填充]
```

</details>

### 结论

- [ ] 双重编译已消除（每个 regex 字符串在整条代码路径中只调用一次 `Regex::new()`）
- [ ] hyperfine 数据已记录
```

---

## Shared Patterns

### 错误类型：ConfigError::InvalidValue
**Source:** `src/error.rs` L54-59，`src/features/filters.rs` L348-362
**Apply to:** `compile_patterns(field, patterns)` 新签名内，所有 `Regex::new()` 失败时

```rust
crate::error::Error::Config(crate::error::ConfigError::InvalidValue {
    field: field.to_string(),
    value: pattern.clone(),
    reason: format!("invalid regex: {e}"),
})
```

### `?` 传播链
**Source:** `src/config.rs` L54-62（`Config::validate` 每个子校验都用 `?`）
**Apply to:** `try_from_meta` → `compile_patterns` 的所有调用点；`build_pipeline` → `FilterProcessor::try_new`

```rust
// 调用链：Config::validate → try_from_meta → compile_patterns → Regex::new → ?
pub fn validate(&self) -> Result<()> {
    self.logging.validate()?;
    // ...
}
```

### fire-and-forget thread::spawn
**Source:** `src/main.rs` L203-208（Ctrl+C handler 注册后不存 JoinHandle）
**Apply to:** `check_for_updates_at_startup()` 改造后不返回 JoinHandle，调用方无需 `.join()`

### 测试中构造 `ConfigError` 的模式
**Source:** `src/config.rs` L706-719（`test_validate_invalid_regex_in_filters` 测试）
**Apply to:** `test_exclude_invalid_regex_validate_fails`（L1278）改为测试 `try_from_meta().is_err()`：

```rust
// 原测试：feature.validate_regexes().is_err()
// 改后：
let result = CompiledMetaFilters::try_from_meta(&feature.meta);
assert!(result.is_err());
```

---

## No Analog Found

无。Phase 9 所有变更都是已有文件内的重构，无新文件，无需从外部引入模式。

---

## Metadata

**Analog search scope:** `src/features/filters.rs`, `src/config.rs`, `src/cli/update.rs`, `src/main.rs`, `src/error.rs`, `src/cli/run.rs`, `benches/BENCHMARKS.md`
**Files scanned:** 7
**Pattern extraction date:** 2026-05-11

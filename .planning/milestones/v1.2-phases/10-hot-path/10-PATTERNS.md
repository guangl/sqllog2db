# Phase 10: 热路径优化 - Pattern Map

**Mapped:** 2026-05-14
**Files analyzed:** 3 (1 新增场景文件 bench_filters.rs、1 文档文件 BENCHMARKS.md、1 可能修改的源文件 src/features/filters.rs)
**Analogs found:** 3 / 3

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `benches/bench_filters.rs` | benchmark | batch (add 2 scenarios) | `benches/bench_filters.rs` itself (existing 5 scenarios) | exact |
| `benches/BENCHMARKS.md` | doc | — (write Phase 10 section) | `benches/BENCHMARKS.md` Phase 9 section | exact |
| `src/features/filters.rs` | service / hot-path logic | request-response (per-record) | `src/features/filters.rs` (FilterProcessor, CompiledMetaFilters) | exact (self-analog) |

---

## Pattern Assignments

### `benches/bench_filters.rs` — 新增 `exclude_passthrough` / `exclude_active` 两个场景

**Analog:** 同文件现有配置函数（`cfg_pipeline_passthrough`、`cfg_trxid_small`）

**Imports pattern** (lines 14-21): 无需新增 import，沿用现有全部 use 语句。

**Config 函数 pattern** (lines 71-127):

每个场景对应一个 `cfg_xxx()` 函数，签名：
```rust
fn cfg_xxx(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let toml = format!(
        "{base}\n[features.filters]\nenable = true\n...",
        base = base_toml(sqllog_dir, bench_dir)
    );
    toml::from_str(&toml).unwrap()
}
```

**`exclude_passthrough` 场景** — 按 D-B3：
```rust
/// exclude 配置存在但无记录命中（纯排除过滤开销）。
/// synthetic_log 中 username 固定为 "BENCH"，
/// exclude 配置为 ["BENCH_EXCLUDE"] → 零命中。
fn cfg_exclude_passthrough(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let toml = format!(
        "{base}
[features.filters]
enable = true
exclude_usernames = [\"BENCH_EXCLUDE\"]
",
        base = base_toml(sqllog_dir, bench_dir)
    );
    toml::from_str(&toml).unwrap()
}
```

**`exclude_active` 场景** — 按 D-B3：
```rust
/// exclude 命中所有记录（100% hit rate）— OR-veto 极端压力场景。
/// exclude 配置为 ["BENCH"]，synthetic_log 中所有记录 username = "BENCH"，全部被排除。
fn cfg_exclude_active(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let toml = format!(
        "{base}
[features.filters]
enable = true
exclude_usernames = [\"BENCH\"]
",
        base = base_toml(sqllog_dir, bench_dir)
    );
    toml::from_str(&toml).unwrap()
}
```

**scenarios 列表扩展 pattern** (lines 135-147):
```rust
let scenarios: &[(&str, Config)] = &[
    // ... 现有 5 个场景 ...
    ("exclude_passthrough", cfg_exclude_passthrough(&sqllog_dir, &bench_dir)),
    ("exclude_active",      cfg_exclude_active(&sqllog_dir, &bench_dir)),
];
```

**bench runner pattern** (lines 152-173): 不变，复用现有 `b.iter_with_setup` + `handle_run` 调用，无需修改 runner 逻辑。

`handle_run` 调用签名（固定，勿改）：
```rust
handle_run(
    cfg,
    None,
    false,
    true,  // quiet=true
    &Arc::new(AtomicBool::new(false)),
    80,
    false,
    None,
    1,
    compiled_filters,
)
.unwrap();
```

---

### `benches/BENCHMARKS.md` — Phase 10 节

**Analog:** Phase 9 节（文件 lines 316-388）和 Phase 5 节（lines 204-282）

**节头 pattern** (Phase 9 lines 316-320):
```markdown
## Phase 10 — 热路径优化（samply + criterion）

**Date:** YYYY-MM-DD
**Goal:** samply profile + exclude benchmark 场景补全；按门控标准（D-G1）判断是否实施优化
**Test environment:** Apple Silicon (Darwin 25.4.0), release build (...)
```

**两个分支写法（D-G3 规定）:**

分支 A — 无符合条件热点（>5% self time 的 src/ 函数不存在或无明确优化路径）：
```markdown
### samply Profiling 结论

Top N 函数（self time 占比，来自 `samply record ./target/release/sqllog2db run -c config.toml`）：
1. `<function>` — X.X%（第三方库内部，不可消除）
2. ...

**结论：已达当前瓶颈。** 无符合 D-G1 全部三条的热点函数（>5% self time + src/ 业务逻辑 + 明确优化路径）。
当前性能受限于第三方解析库（dm-database-parser-sqllog）和系统 mmap I/O，属于不可进一步优化范畴。

### Filter Benchmark（Phase 10 新增场景）

| Scenario              | Median time | Throughput    | Notes |
|-----------------------|------------:|--------------:|-------|
| `exclude_passthrough` |   X.XX ms   |   X.XX M/s    | exclude 配置存在但零命中 |
| `exclude_active`      |   X.XX ms   |   X.XX M/s    | 所有记录被 OR-veto 排除（100% hit rate）|
```

分支 B — 有符合条件热点（实施优化后补写，格式参照 Phase 4 节的各 Wave 数值表 + Criterion 原文）：
```markdown
### samply Profiling 结论

Top N 函数：...（同分支 A 格式）

**热点函数：** `src/xxx/yyy.rs::function_name` — X.X% self time
**优化内容：** （简短说明）

### 优化前/后 Criterion 对比

| Scenario | 优化前 median | 优化后 median | 变化 |
|----------|-------------|-------------|------|
| ...      | ...         | ...         | ...  |
```

**结论清单 pattern** (参照 Phase 9 lines 381-388):
```markdown
### 结论

- [x/[ ]] D-B1 exclude_passthrough / exclude_active 两场景已补全
- [x/[ ]] samply profile 已完成，top N 函数已记录
- [x/[ ]] D-G1 门控判断已执行（有/无热点二选一）
- [x/[ ]] 若有热点：criterion 优化前/后 throughput 数据已记录
- [x/[ ]] 若无热点：已记录"已达当前瓶颈"结论
- [x/[ ]] cargo test 全量通过，clippy/fmt 净化
```

---

### `src/features/filters.rs` — 条件性优化目标

**Analog:** 文件自身现有 hot-path 方法（samply 告知热点前不预设具体修改）

**热路径关键路径** (lines 395-480):

`CompiledMetaFilters::should_keep` → `exclude_veto` → `include_and`，是热路径最终判断函数。

```rust
// should_keep (line 395-402)
#[inline]
#[must_use]
pub fn should_keep(&self, meta: &RecordMeta) -> bool {
    if self.exclude_veto(meta) {
        return false;
    }
    self.include_and(meta)
}
```

`exclude_veto` OR-veto 逻辑 (lines 405-443)：各 exclude 字段独立 `is_some()` guard + `match_any_regex` 模式：
```rust
fn exclude_veto(&self, meta: &RecordMeta) -> bool {
    if self.exclude_usernames.is_some()
        && match_any_regex(self.exclude_usernames.as_deref(), meta.user)
    {
        return true;
    }
    // ... 同模式重复 6 次，每次对应一个 exclude 字段
    false
}
```

`match_any_regex` 内联函数 (lines 284-290)：
```rust
#[inline]
fn match_any_regex(patterns: Option<&[Regex]>, val: &str) -> bool {
    match patterns {
        None | Some([]) => true,
        Some(p) => p.iter().any(|re| re.is_match(val)),
    }
}
```

**若 samply 发现热点，可能的优化模式（仅参考，以实际 samply 结果为准）：**

- 减少 `is_some()` 重复调用：将 `exclude_veto` 中每个字段的 `is_some()` 检查合并为一个预计算布尔标志（类似 `has_meta_filters` 字段，lines 43-44）
- 减少 `include_and` 中 `as_deref()` 链调用开销（如果 samply 指向此处）

**修改约束（D-O3）：**
- 修改 `src/` 时必须通过 `cargo clippy --all-targets -- -D warnings`
- `cargo test` 全量通过
- criterion 新 baseline 不低于优化前（throughput 无回归）

---

## Shared Patterns

### Benchmark 配置函数模式
**Source:** `benches/bench_filters.rs` lines 66-127
**Apply to:** 新增的 `cfg_exclude_passthrough` 和 `cfg_exclude_active`

公共规则：
1. 函数签名：`fn cfg_xxx(sqllog_dir: &Path, bench_dir: &Path) -> Config`
2. 配置通过 `format!("{base}\n...", base = base_toml(...))` 拼接 TOML 字符串
3. 用 `toml::from_str(&toml).unwrap()` 反序列化，不 unwrap_or

### Filter TOML 字段名规范
**Source:** `src/features/filters.rs` lines 74-80 (`MetaFilters` 定义)

exclude 字段名（serde 名称）：
```
exclude_usernames, exclude_client_ips, exclude_sess_ids,
exclude_thrd_ids, exclude_statements, exclude_appnames, exclude_tags
```
D-B2 规定只需 `exclude_usernames` 字段即可代表全局（内部路径等价）。

### handle_run 调用签名
**Source:** `benches/bench_filters.rs` lines 157-169
**Apply to:** 所有 bench 场景 runner

`handle_run` 参数顺序（9 个固定参数 + `compiled_filters`）：
```rust
handle_run(cfg, None, false, true, &Arc::new(AtomicBool::new(false)), 80, false, None, 1, compiled_filters)
```
`compiled_filters` 来自 `iter_with_setup` 的 setup 闭包：`cfg.validate_and_compile().unwrap()`。

### BENCHMARKS.md 节格式
**Source:** `benches/BENCHMARKS.md` lines 316-388 (Phase 9 节)
**Apply to:** Phase 10 节

结构：`## Phase X — 标题` → `**Date/Goal/Test environment**` → `### 子节` → `### 结论` checkbox 列表。
Criterion 原始输出用 `<details><summary>...</summary>` 折叠。

---

## No Analog Found

无。所有涉及文件均有明确类比（bench_filters.rs 自身现有场景即为最优类比）。

---

## Metadata

**Analog search scope:** `benches/`, `src/features/`, `src/cli/`
**Files scanned:** 4 (`bench_filters.rs`, `filters.rs`, `run.rs`, `BENCHMARKS.md`)
**Pattern extraction date:** 2026-05-14

# Phase 2: 输出字段控制 - Research

**Researched:** 2026-04-18
**Domain:** Rust / exporter 层字段投影（CSV + SQLite），有序索引接线
**Confidence:** HIGH

---

## Summary

Phase 2 的核心任务是让 `FieldMask` 从"仅过滤哪些字段"升级为"按用户指定顺序输出字段"。通过对源代码的全面审阅，发现现有代码已高度预置：`FieldMask`、`field_mask()`、字段名合法性校验、以及两个 exporter 的 bitmask 投影路径均已实现。唯一缺失的是**有序索引**支持——当前 `build_header()`（CSV）和 `build_create_sql()`/`build_insert_sql()`（SQLite）都按 `FIELD_NAMES` 原始顺序（0→14）遍历 bitmask，而不是按用户配置顺序遍历。

所需改动精确且集中：在 `FeaturesConfig` 中新增 `ordered_field_indices()` 方法，返回 `Vec<usize>`（按用户配置顺序的字段索引列表），再将两个 exporter 的 header/schema/data 写入逻辑改为按此有序列表遍历。`handle_run` 和并行路径已正确传递 `field_mask`，但不需要单独传递 `ordered_indices`——有序索引可在 exporter 初始化时通过新方法计算并存储到 exporter 字段中。

整个 Phase 2 无需引入新依赖，无需修改 `Pipeline`、过滤器、或 `handle_run` 的主流程逻辑。预计改动文件：`src/features/mod.rs`（新增方法）、`src/exporter/csv.rs`（header + data 写入顺序）、`src/exporter/sqlite.rs`（CREATE TABLE + INSERT 列顺序 + Value 选取顺序）、`src/exporter/mod.rs`（`from_config` 接线新字段）。

**Primary recommendation:** 在 exporter 中存储 `ordered_indices: Vec<usize>`（不替换 `field_mask`，与之并存），由 `FeaturesConfig::ordered_field_indices()` 计算，在 `from_config` 时注入。

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** 列顺序按用户配置顺序输出（而非固定原始顺序）。需在 FieldMask bitmask 之外额外存储 `Vec<usize>` 有序字段索引列表，供 exporter 按顺序写入列。
- **D-02:** `features.fields = []` 等同于不配置（导出全部字段），不报错，零歧义。
- **D-03:** fields 列表中未包含 `normalized_sql` 时，即使 `replace_parameters.enable = true`，也静默忽略（不导出 normalized_sql，不给出警告）。replace_parameters 功能照常执行，结果在写入阶段丢弃。

### Claude's Discretion

- FieldMask 与有序索引的具体数据结构（`Vec<usize>` 还是其他形式）由实现决定，需确保与现有 FieldMask API 向后兼容。
- CSV header 行和 SQLite 建表语句中的列名顺序均跟随有序索引列表。

### Deferred Ideas (OUT OF SCOPE)

None — 讨论严格在 Phase 2 范围内进行。
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FIELD-01 | 用户可在 config.toml 中指定导出哪些字段（列名列表），未指定则导出全部字段 | `FeaturesConfig.fields` 配置字段已存在；`FieldMask` 已实现；需新增 `ordered_field_indices()` 并接线到两个 exporter |
</phase_requirements>

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 字段名合法性校验 | Config 层（`Config::validate()`）| — | 已实现，无需修改；启动阶段报错符合现有模式 |
| 有序索引计算 | Features 层（`FeaturesConfig`）| — | 字段配置属于 features 概念；与 `field_mask()` 对称放置 |
| CSV 列顺序控制 | Exporter 层（`CsvExporter`）| — | header + data 写入均在此层；`ordered_indices` 存储于 exporter |
| SQLite 列顺序控制 | Exporter 层（`SqliteExporter`）| — | CREATE TABLE / INSERT 列顺序均在此层 |
| 注入 ordered_indices | ExporterManager::from_config | — | 已是 exporter 初始化的统一入口 |
| 并行 CSV 路径接线 | cli/run.rs（`process_csv_parallel`）| — | 已传递 `field_mask`；需同样传递 `ordered_indices` 给临时 CsvExporter |

---

## Standard Stack

### 已使用（无需引入新依赖）

| 组件 | 版本 | 用途 | 状态 |
|------|------|------|------|
| `FieldMask` | — | u16 bitmask，`is_active(idx)` | 已实现，继续使用 |
| `FIELD_NAMES: &[&str]` | — | 15 个字段名，顺序即原始列定义顺序 | 继续使用 |
| `itoa` | 已锁定 | CSV 数字格式化，零分配 | 无需修改 |
| `memchr` | 已锁定 | CSV 引号转义快速扫描 | 无需修改 |
| `rusqlite` | 已锁定 | SQLite INSERT/CREATE | 投影路径已有 `params_from_iter` |

[VERIFIED: 通过直接阅读 Cargo.toml 和源代码]

---

## Architecture Patterns

### System Architecture Diagram

```
config.toml: features.fields = ["sql", "username", "ts"]
    |
    v
Config::validate()              ← 启动阶段：字段名合法性检查（已实现）
    |
    v
FeaturesConfig::ordered_field_indices()  ← 新增：返回 Vec<usize> = [10, 4, 0]
FeaturesConfig::field_mask()             ← 已实现：返回 FieldMask(bitmask)
    |
    v
ExporterManager::from_config()
    |
    ├── CsvExporter { field_mask, ordered_indices }   ← 新增 ordered_indices 字段
    |       |
    |       ├── initialize() → build_header()         ← 按 ordered_indices 顺序写列名
    |       └── export_one_preparsed() → write_record_preparsed()  ← 按 ordered_indices 顺序写值
    |
    └── SqliteExporter { field_mask, ordered_indices } ← 新增 ordered_indices 字段
            |
            ├── initialize() → build_create_sql()     ← 按 ordered_indices 顺序建列
            ├── initialize() → build_insert_sql()     ← 按 ordered_indices 顺序命名列
            └── do_insert_preparsed()                 ← 按 ordered_indices 顺序选取 Value

parallel path: process_csv_parallel()
    |
    └── CsvExporter::new() + exporter.ordered_indices = ordered_indices  ← 需接线
```

### Recommended Project Structure（无变化）

```
src/
├── features/mod.rs      — 新增 ordered_field_indices() 方法
├── exporter/
│   ├── mod.rs           — from_config 接线 ordered_indices
│   ├── csv.rs           — 新增 ordered_indices 字段，修改 header/data 写入顺序
│   └── sqlite.rs        — 新增 ordered_indices 字段，修改 CREATE/INSERT/Value 顺序
└── cli/run.rs           — process_csv_parallel 传递 ordered_indices
```

### Pattern 1: ordered_field_indices() 方法设计

**What:** 从 `features.fields: Option<Vec<String>>` 计算有序字段索引列表

**When to use:** exporter 初始化时，由 `from_config` 调用

```rust
// src/features/mod.rs — 新增方法
impl FeaturesConfig {
    /// 按用户配置顺序返回字段索引列表。
    /// - None 或空列表 → [0, 1, ..., 14]（全量原始顺序）
    /// - 有效列表 → 按配置顺序的字段索引（字段名已在 validate() 阶段验证）
    #[must_use]
    pub fn ordered_field_indices(&self) -> Vec<usize> {
        match &self.fields {
            None => (0..FIELD_NAMES.len()).collect(),
            Some(names) if names.is_empty() => (0..FIELD_NAMES.len()).collect(),  // D-02
            Some(names) => names
                .iter()
                .filter_map(|name| FIELD_NAMES.iter().position(|&n| n == name.as_str()))
                .collect(),
        }
    }
}
```

[VERIFIED: 基于对 FeaturesConfig 和 FIELD_NAMES 的直接阅读]

### Pattern 2: CsvExporter 有序 header 构建

**What:** 将现有 `build_header()` 从"遍历 bitmask"改为"遍历 ordered_indices"

现有代码问题：
```rust
// 当前实现（按原始顺序遍历）
for (i, name) in FIELD_NAMES.iter().enumerate() {
    if self.field_mask.is_active(i) { /* 写入 */ }
}
```

改后模式：
```rust
// 新实现（按用户配置顺序遍历）
for &idx in &self.ordered_indices {
    // idx 14 (normalized_sql) 在 normalize=false 时跳过
    if idx == 14 && !self.normalize {
        continue;
    }
    if !first { header.push(b','); }
    first = false;
    header.extend_from_slice(FIELD_NAMES[idx].as_bytes());
}
```

[VERIFIED: 基于对 csv.rs build_header() 的直接阅读]

### Pattern 3: CsvExporter 有序数据行写入

**What:** `write_record_preparsed` 的投影路径改为按 `ordered_indices` 顺序写字段

现有投影路径用 15 个独立 `if field_mask.is_active(N)` 块按固定顺序写入。改后需按 `ordered_indices` 遍历，根据 idx 分发到对应字段值。

推荐实现：提取各字段值为局部变量或使用 `match idx` 分发：

```rust
// 投影路径：按 ordered_indices 顺序写入
for &idx in ordered_indices {
    if need_sep { line_buf.push(b','); }
    need_sep = true;
    match idx {
        0  => line_buf.extend_from_slice(sqllog.ts.as_ref().as_bytes()),
        1  => line_buf.extend_from_slice(itoa_buf.format(meta.ep).as_bytes()),
        // ... 14 个 match arm
        14 => { /* normalized_sql，仅当 normalize=true */ }
        _  => {}
    }
}
```

注意：全量掩码快速路径（`field_mask == FieldMask::ALL`）保留不变，性能不回退。

[VERIFIED: 基于对 csv.rs write_record_preparsed() 的直接阅读]

### Pattern 4: SQLite 有序 CREATE TABLE 和 INSERT

**What:** `build_create_sql()` 和 `build_insert_sql()` 改为按 ordered_indices 顺序

```rust
// build_create_sql 改后
fn build_create_sql(table_name: &str, ordered_indices: &[usize]) -> String {
    let cols: Vec<String> = ordered_indices
        .iter()
        .map(|&i| format!("{} {}", FIELD_NAMES[i], COL_TYPES[i]))
        .collect();
    format!("CREATE TABLE IF NOT EXISTS {table_name} ({})", cols.join(", "))
}

// build_insert_sql 改后
fn build_insert_sql(table_name: &str, ordered_indices: &[usize]) -> String {
    if ordered_indices.len() == FIELD_NAMES.len() {
        // 全量快速路径
        return format!("INSERT INTO {table_name} VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)");
    }
    let cols: Vec<&str> = ordered_indices.iter().map(|&i| FIELD_NAMES[i]).collect();
    let placeholders = vec!["?"; ordered_indices.len()].join(", ");
    format!("INSERT INTO {table_name} ({}) VALUES ({placeholders})", cols.join(", "))
}
```

[VERIFIED: 基于对 sqlite.rs build_create_sql() 和 build_insert_sql() 的直接阅读]

### Pattern 5: SQLite do_insert_preparsed Value 选取顺序

现有投影路径按 bitmask 顺序过滤 `[Value; 15]`：

```rust
// 当前：filter 按原始顺序，再按 field_mask 选
let selected: Vec<Value> = all.into_iter().enumerate()
    .filter(|(i, _)| field_mask.is_active(*i))
    .map(|(_, v)| v)
    .collect();
```

改后：按 `ordered_indices` 选取（需先 ref，不能 move）：

```rust
// 新实现：按有序索引选取
let all: [Value; 15] = [...]; // 同现有，构建全量数组
let selected: Vec<&Value> = ordered_indices.iter().map(|&i| &all[i]).collect();
stmt.execute(rusqlite::params_from_iter(selected))?;
```

[VERIFIED: 基于对 sqlite.rs do_insert_preparsed() 的直接阅读]

### Anti-Patterns to Avoid

- **不要把有序索引存到 `FieldMask` 里：** `FieldMask` 是 `Copy` 类型（u16），将 `Vec<usize>` 嵌入会破坏现有所有调用点（大量 `Copy` 语义依赖）。保持两个字段并存。
- **不要在热路径里重新计算 ordered_indices：** 计算一次存入 exporter 字段，初始化后不变。
- **不要删除全量掩码快速路径：** `field_mask == FieldMask::ALL` 的快速路径对性能关键，必须保留。全量路径下 `ordered_indices` 自然是 `[0..14]`，快速路径代码不需要使用它。
- **并行路径不要遗漏：** `process_csv_parallel` 中对每个临时文件创建 `CsvExporter::new()` 后直接设置 `field_mask`，需同样设置 `ordered_indices`。

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SQLite 动态参数绑定 | 手写 format SQL 字符串拼接参数 | `rusqlite::params_from_iter` | 已使用，防 SQL 注入，类型安全 |
| 字段名合法性校验 | 重新实现验证逻辑 | 现有 `Config::validate()` 中的校验 | 已实现且有测试，无需触碰 |
| FieldMask 构建 | 重新计算 bitmask | 现有 `FieldMask::from_names()` | 已实现，有错误处理 |

---

## Common Pitfalls

### Pitfall 1: 并行路径遗漏 ordered_indices 注入

**What goes wrong:** 顺序路径（`ExporterManager::from_config`）接线了 `ordered_indices`，但并行路径（`process_csv_parallel` 中 `CsvExporter::new()` 后手动设置字段）没有同步设置，导致并行时列顺序退化为原始顺序。

**Why it happens:** 并行路径绕过 `ExporterManager::from_config`，直接构建 `CsvExporter` 并设置 `field_mask`。

**How to avoid:** 在 `process_csv_parallel` 的 `exporter.field_mask = field_mask;` 行旁边同时设置 `exporter.ordered_indices = ordered_indices.clone();`，并在函数签名中增加 `ordered_indices: &[usize]` 参数。

**Warning signs:** 集成测试中"指定字段 + 并行模式"时列顺序不对。

### Pitfall 2: SQLite Value 数组索引越界

**What goes wrong:** `do_insert_preparsed` 构建 `[Value; 15]` 全量数组后，按 `ordered_indices` 取元素。若 ordered_indices 包含 >= 15 的值，会 panic。

**Why it happens:** `ordered_field_indices()` 理论上只返回 `[0, 14]` 范围内的索引（因为 `FIELD_NAMES.len() == 15`），但如果实现有 off-by-one 就会越界。

**How to avoid:** `ordered_field_indices()` 的 `position()` 调用天然限制在 `[0, FIELD_NAMES.len()-1]`，无需额外边界检查。写单元测试验证边界（仅含最后一个字段 `normalized_sql`）。

### Pitfall 3: normalized_sql 字段在 CSV 全量快速路径中的特殊处理

**What goes wrong:** 全量掩码（`FieldMask::ALL`）快速路径在 `normalize=false` 时不写 `normalized_sql`。如果用户配置 `fields` 中包含 `normalized_sql` 但 `normalize=false`，有序路径需要同样跳过它。

**Why it happens:** `normalize` 标志和 `field_mask` 是两个独立控制，`build_header` 已经处理了这个组合（`if i == 14 && !self.normalize { continue; }`），data 写入路径也需同样处理。

**How to avoid:** 在有序 data 写入的 `match idx { 14 => ... }` arm 中检查 `normalize` 标志，与 header 保持一致逻辑。

### Pitfall 4: D-02 空列表与 None 的一致性

**What goes wrong:** `features.fields = []` 应等同于不配置，但若 `ordered_field_indices()` 对空列表返回空 `Vec`，exporter 会写出零列的 CSV/SQLite，破坏功能。

**Why it happens:** 实现者只处理了 `None` 分支，没有单独处理 `Some(Vec::new())`。

**How to avoid:** `ordered_field_indices()` 在 `Some(names) if names.is_empty()` 时返回全量 `(0..15).collect()`，与 `None` 完全一致。

---

## Code Examples

### 完整 ordered_field_indices() 实现

```rust
// src/features/mod.rs
impl FeaturesConfig {
    /// 按用户配置顺序返回字段索引列表。
    /// None 或空列表返回全量原始顺序 [0..14]（D-02 决策）。
    #[must_use]
    pub fn ordered_field_indices(&self) -> Vec<usize> {
        match &self.fields {
            None => (0..FIELD_NAMES.len()).collect(),
            Some(names) if names.is_empty() => (0..FIELD_NAMES.len()).collect(),
            Some(names) => names
                .iter()
                .filter_map(|name| FIELD_NAMES.iter().position(|&n| n == name.as_str()))
                .collect(),
        }
    }
}
```

### CsvExporter 新增字段及初始化

```rust
pub struct CsvExporter {
    // ... 现有字段 ...
    pub(crate) field_mask: crate::features::FieldMask,
    pub(crate) ordered_indices: Vec<usize>,  // 新增
}

impl CsvExporter {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            // ... 现有字段初始化 ...
            field_mask: crate::features::FieldMask::ALL,
            ordered_indices: (0..crate::features::FIELD_NAMES.len()).collect(), // 默认全量
        }
    }
}
```

### ExporterManager::from_config 接线

```rust
// src/exporter/mod.rs
pub fn from_config(config: &Config) -> Result<Self> {
    let normalize = ...;
    let field_mask = config.features.field_mask();
    let ordered_indices = config.features.ordered_field_indices(); // 新增

    if let Some(cfg) = &config.exporter.csv {
        let mut exporter = CsvExporter::from_config(cfg);
        exporter.normalize = normalize;
        exporter.field_mask = field_mask;
        exporter.ordered_indices = ordered_indices; // 新增
        return Ok(Self { exporter: ExporterKind::Csv(exporter) });
    }

    if let Some(cfg) = &config.exporter.sqlite {
        let mut exporter = SqliteExporter::from_config(cfg);
        exporter.normalize = normalize;
        exporter.field_mask = field_mask;
        exporter.ordered_indices = ordered_indices; // 新增
        return Ok(Self { exporter: ExporterKind::Sqlite(exporter) });
    }
    // ...
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 按原始顺序遍历 bitmask | 按用户配置有序索引遍历 | Phase 2 实现后 | 用户可自定义列顺序 |
| field_mask 控制一切投影 | field_mask + ordered_indices 并存 | Phase 2 实现后 | field_mask 继续用于 ALL 快速路径判断 |

**不废弃：**
- `FieldMask::ALL` 快速路径：全量输出时性能不回退
- `FeaturesConfig::field_mask()`：`handle_run` 和并行路径仍用它判断 `do_normalize`（`includes_normalized_sql()`）

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| — | — | — | — |

**此表为空：** 所有关键实现细节均通过直接阅读源代码验证，无假设。

---

## Open Questions

无阻塞性问题。以下为实现时需决策的细节（Claude's Discretion 范围内）：

1. **并行路径函数签名**
   - 当前 `process_csv_parallel` 接受 `field_mask: FieldMask`（`Copy`）
   - 新增 `ordered_indices: &[usize]`（借用切片，避免 clone）或 `ordered_indices: Vec<usize>` 再 clone 给每个任务
   - **建议：** 传 `&[usize]`，每个并行任务 `to_vec()` 得到独立所有权

---

## Environment Availability

Step 2.6: SKIPPED — Phase 2 为纯代码修改，无外部工具依赖。现有 Rust 工具链和所有 crate 依赖已在 Phase 1 验证可用。

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test（`#[test]`）|
| Config file | 无独立配置，`cargo test` 即可 |
| Quick run command | `cargo test -q 2>&1 \| tail -5` |
| Full suite command | `cargo test && cargo clippy --all-targets -- -D warnings` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FIELD-01-A | `fields = ["sql", "ts"]` → CSV header 只含 sql,ts，顺序与配置一致 | unit | `cargo test test_csv_field_order` | ❌ Wave 0 |
| FIELD-01-B | `fields = ["sql", "ts"]` → SQLite CREATE TABLE 列顺序与配置一致 | unit | `cargo test test_sqlite_field_order` | ❌ Wave 0 |
| FIELD-01-C | `fields` 未配置 → 输出全部 15 列，行为不变（回归） | unit | `cargo test test_csv_basic_export` | ✅ 已有 |
| FIELD-01-D | `fields = []` → 等同全量输出（D-02）| unit | `cargo test test_ordered_indices_empty_equals_all` | ❌ Wave 0 |
| FIELD-01-E | `ordered_field_indices()` 单元测试：None/空/有序/重复字段 | unit | `cargo test test_ordered_field_indices` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -q`
- **Per wave merge:** `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `src/features/mod.rs` 中 `ordered_field_indices()` 的单元测试
- [ ] `src/exporter/csv.rs` 中字段顺序集成测试（`test_csv_field_order`）
- [ ] `src/exporter/sqlite.rs` 中字段顺序集成测试（`test_sqlite_field_order`）
- [ ] D-02 空列表等同全量的测试

---

## Security Domain

本 Phase 无新增 API 端点、无用户输入直接进入 SQL 执行路径（`FIELD_NAMES` 是静态常量，字段名已在 `validate()` 阶段对比白名单验证）。SQLite INSERT 列名来自 `FIELD_NAMES[i]`（静态字符串），不存在 SQL 注入风险。无需额外 ASVS 控制。

---

## Sources

### Primary (HIGH confidence)

- `src/features/mod.rs`（直接阅读）— `FieldMask`、`FIELD_NAMES`、`FeaturesConfig::field_mask()`
- `src/exporter/csv.rs`（直接阅读）— `build_header()`、`write_record_preparsed()`、投影路径
- `src/exporter/sqlite.rs`（直接阅读）— `build_create_sql()`、`build_insert_sql()`、`do_insert_preparsed()`
- `src/exporter/mod.rs`（直接阅读）— `ExporterManager::from_config()`
- `src/cli/run.rs`（直接阅读）— `handle_run()`、`process_csv_parallel()`
- `src/config.rs`（直接阅读）— `Config::validate()` 字段名校验

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — 全部通过源代码直接验证，无假设
- Architecture: HIGH — 改动点精确定位到具体函数，有代码示例
- Pitfalls: HIGH — 基于对现有代码模式的理解，非推测

**Research date:** 2026-04-18
**Valid until:** 代码结构稳定，30 天内有效

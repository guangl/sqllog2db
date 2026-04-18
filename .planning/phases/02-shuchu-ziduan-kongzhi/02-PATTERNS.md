# Phase 2: 输出字段控制 - Pattern Map

**Mapped:** 2026-04-18
**Files analyzed:** 4 modified files
**Analogs found:** 4 / 4 (全部为 exact role-match，同文件扩展)

---

## File Classification

| 修改文件 | Role | Data Flow |最近 Analog | 匹配质量 |
|----------|------|-----------|-------------|----------|
| `src/features/mod.rs` | utility / config | transform | 同文件 `field_mask()` 方法（lines 131-137） | exact |
| `src/exporter/mod.rs` | factory | request-response | 同文件 `from_config()`（lines 201-233） | exact |
| `src/exporter/csv.rs` | exporter | streaming / file-I/O | 同文件 `build_header()` + `write_record_preparsed()`（lines 274-293, 76-243） | exact |
| `src/exporter/sqlite.rs` | exporter | CRUD / batch | 同文件 `build_create_sql()` + `build_insert_sql()` + `do_insert_preparsed()`（lines 51-101, 121-188） | exact |
| `src/cli/run.rs` | orchestrator | request-response | 同文件 `process_csv_parallel()` 签名 + `handle_run` field_mask 传递（lines 424-436, 640-688） | exact |

---

## Pattern Assignments

### `src/features/mod.rs` — 新增 `ordered_field_indices()` 方法

**Analog（同文件）:** `FeaturesConfig::field_mask()`（lines 131-137）

**结构参考模式**（lines 128-137）:
```rust
impl FeaturesConfig {
    /// 计算字段投影掩码。字段名在 `validate()` 阶段已验证，无效名称静默退化为全量掩码。
    #[must_use]
    pub fn field_mask(&self) -> FieldMask {
        match &self.fields {
            None => FieldMask::ALL,
            Some(names) => FieldMask::from_names(names).unwrap_or(FieldMask::ALL),
        }
    }
}
```

**新增方法紧跟 `field_mask()` 之后，结构完全对称：**
```rust
/// 按用户配置顺序返回字段索引列表。
/// - None 或空列表 → [0, 1, ..., 14]（全量原始顺序，对应 D-02）
/// - 有效列表 → 按配置顺序的字段索引（字段名已在 validate() 阶段验证）
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
```

**测试模式**（参考同文件 lines 252-272 中 `test_features_config_default` 等单元测试结构）：
```rust
#[test]
fn test_ordered_field_indices_none_returns_all() {
    let cfg = FeaturesConfig::default();
    let indices = cfg.ordered_field_indices();
    assert_eq!(indices, (0..15).collect::<Vec<_>>());
}

#[test]
fn test_ordered_field_indices_empty_equals_all() {
    let cfg = FeaturesConfig { fields: Some(vec![]), ..Default::default() };
    let indices = cfg.ordered_field_indices();
    assert_eq!(indices.len(), 15);  // D-02
}

#[test]
fn test_ordered_field_indices_preserves_user_order() {
    let cfg = FeaturesConfig {
        fields: Some(vec!["sql".into(), "username".into(), "ts".into()]),
        ..Default::default()
    };
    let indices = cfg.ordered_field_indices();
    assert_eq!(indices, vec![10, 4, 0]);  // sql=10, username=4, ts=0
}
```

---

### `src/exporter/mod.rs` — `from_config()` 接线 `ordered_indices`

**Analog（同文件）:** `from_config()` 中 `field_mask` 注入模式（lines 201-233）

**现有 field_mask 注入模式**（lines 210-227）:
```rust
pub fn from_config(config: &Config) -> Result<Self> {
    let normalize = config.features.replace_parameters.as_ref().is_none_or(|r| r.enable);
    let field_mask = config.features.field_mask();  // ← 已有

    if let Some(cfg) = &config.exporter.csv {
        let mut exporter = CsvExporter::from_config(cfg);
        exporter.normalize = normalize;
        exporter.field_mask = field_mask;           // ← 注入模式
        return Ok(Self { exporter: ExporterKind::Csv(exporter) });
    }

    if let Some(cfg) = &config.exporter.sqlite {
        let mut exporter = SqliteExporter::from_config(cfg);
        exporter.normalize = normalize;
        exporter.field_mask = field_mask;           // ← 注入模式
        return Ok(Self { exporter: ExporterKind::Sqlite(exporter) });
    }
    // ...
}
```

**新增 `ordered_indices` 注入，完全复制 `field_mask` 的注入方式：**
```rust
let field_mask = config.features.field_mask();
let ordered_indices = config.features.ordered_field_indices();  // 新增，紧跟 field_mask 之后

// CSV 分支：
exporter.field_mask = field_mask;
exporter.ordered_indices = ordered_indices.clone();  // 新增

// SQLite 分支：
exporter.field_mask = field_mask;
exporter.ordered_indices = ordered_indices;          // 新增（最后一个分支无需 clone）
```

---

### `src/exporter/csv.rs` — 结构体新增字段 + `build_header()` + `write_record_preparsed()` 投影路径

**Analog（同文件）:** `field_mask` 字段声明（line 32-33）和 `build_header()` 遍历模式（lines 274-293）

**结构体字段声明模式**（lines 23-33，`field_mask` 紧邻 `normalize`）:
```rust
pub struct CsvExporter {
    // ...
    pub(crate) normalize: bool,
    pub(crate) field_mask: crate::features::FieldMask,
    // ordered_indices 新增于此，与 field_mask 相邻
}
```

**`new()` 默认值模式**（lines 44-60，`field_mask: FieldMask::ALL` 对应 `ordered_indices` 默认全量）:
```rust
pub fn new(path: impl AsRef<Path>) -> Self {
    Self {
        // ...
        normalize: true,
        field_mask: crate::features::FieldMask::ALL,
        ordered_indices: (0..crate::features::FIELD_NAMES.len()).collect(),  // 新增
    }
}
```

**`build_header()` 现有遍历模式**（lines 274-293，需改为按 `ordered_indices` 遍历）:
```rust
// 现有：按原始枚举顺序遍历 + bitmask 过滤
fn build_header(&self) -> Vec<u8> {
    use crate::features::FIELD_NAMES;
    let mut header = Vec::with_capacity(128);
    let mut first = true;
    for (i, name) in FIELD_NAMES.iter().enumerate() {
        if i == 14 && !self.normalize { continue; }  // normalize 特殊处理
        if self.field_mask.is_active(i) {
            if !first { header.push(b','); }
            first = false;
            header.extend_from_slice(name.as_bytes());
        }
    }
    header.push(b'\n');
    header
}
```

**改后模式（按 `ordered_indices` 遍历，去掉 bitmask 判断）：**
```rust
fn build_header(&self) -> Vec<u8> {
    use crate::features::FIELD_NAMES;
    let mut header = Vec::with_capacity(128);
    let mut first = true;
    for &idx in &self.ordered_indices {
        if idx == 14 && !self.normalize { continue; }   // 保持相同的 normalize 逻辑
        if !first { header.push(b','); }
        first = false;
        header.extend_from_slice(FIELD_NAMES[idx].as_bytes());
    }
    header.push(b'\n');
    header
}
```

**`write_record_preparsed()` 投影路径现有模式**（lines 142-232，15 个顺序 `if field_mask.is_active(N)` 块）：
投影路径的 `w_sep!()` macro 和分支结构已可复用，需改为 `match idx` 分发：
```rust
// 改后：按 ordered_indices 遍历，match idx 分发字段值
// 全量掩码快速路径（lines 98-141）保留不变
} else {
    let mut need_sep = false;
    macro_rules! w_sep {
        () => {
            if need_sep { line_buf.push(b','); }
            need_sep = true;
        };
    }
    let has_metrics = pm.exec_id != 0 || pm.exectime > 0.0;
    for &idx in ordered_indices {
        match idx {
            0  => { w_sep!(); line_buf.extend_from_slice(sqllog.ts.as_ref().as_bytes()); }
            1  => { w_sep!(); line_buf.extend_from_slice(itoa_buf.format(meta.ep).as_bytes()); }
            2  => { w_sep!(); line_buf.extend_from_slice(meta.sess_id.as_ref().as_bytes()); }
            3  => { w_sep!(); line_buf.extend_from_slice(meta.thrd_id.as_ref().as_bytes()); }
            4  => { w_sep!(); line_buf.extend_from_slice(meta.username.as_ref().as_bytes()); }
            5  => { w_sep!(); line_buf.extend_from_slice(meta.trxid.as_ref().as_bytes()); }
            6  => { w_sep!(); line_buf.extend_from_slice(meta.statement.as_ref().as_bytes()); }
            7  => { w_sep!(); line_buf.extend_from_slice(meta.appname.as_ref().as_bytes()); }
            8  => { w_sep!(); line_buf.extend_from_slice(strip_ip_prefix(meta.client_ip.as_ref()).as_bytes()); }
            9  => { w_sep!(); if let Some(tag) = &sqllog.tag { line_buf.extend_from_slice(tag.as_ref().as_bytes()); } }
            10 => { w_sep!(); line_buf.push(b'"'); write_csv_escaped(line_buf, pm.sql.as_bytes()); line_buf.push(b'"'); }
            11 => { w_sep!(); if has_metrics { line_buf.extend_from_slice(itoa_buf.format(f32_ms_to_i64(pm.exectime)).as_bytes()); } }
            12 => { w_sep!(); if has_metrics { line_buf.extend_from_slice(itoa_buf.format(i64::from(pm.rowcount)).as_bytes()); } }
            13 => { w_sep!(); if has_metrics { line_buf.extend_from_slice(itoa_buf.format(pm.exec_id).as_bytes()); } }
            14 => {
                if normalize {                             // D-03：normalize=false 时跳过
                    w_sep!();
                    if let Some(ns) = normalized_sql {
                        line_buf.push(b'"'); write_csv_escaped(line_buf, ns.as_bytes()); line_buf.push(b'"');
                    }
                }
            }
            _ => {}
        }
    }
    let _ = need_sep;
}
```

**`write_record_preparsed` 函数签名** — 新增 `ordered_indices: &[usize]` 参数，与 `field_mask` 相邻（line 87 区域）:
```rust
fn write_record_preparsed(
    // ... 现有参数 ...
    field_mask: crate::features::FieldMask,
    ordered_indices: &[usize],   // 新增，紧跟 field_mask
) -> Result<()>
```

**测试模式**（参考同文件 `test_csv_basic_export`，lines 463-485）：
```rust
#[test]
fn test_csv_field_order() {
    // 配置 fields = ["sql", "username"]，验证 header 为 "sql,username\n"
    let mut exporter = CsvExporter::new(&outfile);
    exporter.field_mask = FieldMask::from_names(&["sql".into(), "username".into()]).unwrap();
    exporter.ordered_indices = vec![10, 4];   // sql=10, username=4
    exporter.initialize().unwrap();
    // 断言 header 第一行为 "sql,username"
}
```

---

### `src/exporter/sqlite.rs` — 结构体新增字段 + `build_create_sql()` + `build_insert_sql()` + `do_insert_preparsed()` 投影路径

**Analog（同文件）:** `field_mask` 字段声明（lines 18-19）和现有 `build_create_sql()` / `build_insert_sql()` / `do_insert_preparsed()` 模式

**结构体字段声明模式**（lines 9-19）：
```rust
pub struct SqliteExporter {
    // ...
    pub(super) normalize: bool,
    pub(super) field_mask: crate::features::FieldMask,
    // ordered_indices 新增于此
}
```

**`build_create_sql()` 现有模式**（lines 72-101，按 bitmask 过滤原始顺序）：
```rust
// 现有：按枚举顺序 + bitmask 过滤
fn build_create_sql(table_name: &str, field_mask: crate::features::FieldMask) -> String {
    let cols: Vec<String> = FIELD_NAMES.iter().enumerate()
        .filter(|(i, _)| field_mask.is_active(*i))
        .map(|(i, name)| format!("{name} {}", COL_TYPES[i]))
        .collect();
    format!("CREATE TABLE IF NOT EXISTS {table_name} ({})", cols.join(", "))
}
```

**改后：签名改为接受 `ordered_indices`，按有序索引构建列定义：**
```rust
fn build_create_sql(table_name: &str, ordered_indices: &[usize]) -> String {
    const COL_TYPES: &[&str] = &[ /* 保持不变，15 个类型定义 */ ];
    let cols: Vec<String> = ordered_indices.iter()
        .map(|&i| format!("{} {}", FIELD_NAMES[i], COL_TYPES[i]))
        .collect();
    format!("CREATE TABLE IF NOT EXISTS {table_name} ({})", cols.join(", "))
}
```

**`build_insert_sql()` 现有模式**（lines 51-69，按 bitmask 过滤）：
```rust
// 现有：全量快速路径 + 投影路径
fn build_insert_sql(table_name: &str, field_mask: crate::features::FieldMask) -> String {
    if field_mask == crate::features::FieldMask::ALL {
        return format!("INSERT INTO {table_name} VALUES (?, ?, ..., ?)");
    }
    let selected: Vec<&str> = FIELD_NAMES.iter().enumerate()
        .filter(|(i, _)| field_mask.is_active(*i))
        .map(|(_, name)| *name).collect();
    let placeholders = vec!["?"; selected.len()].join(", ");
    format!("INSERT INTO {table_name} ({}) VALUES ({placeholders})", selected.join(", "))
}
```

**改后：签名改为接受 `ordered_indices`，保留全量快速路径判断（按长度）：**
```rust
fn build_insert_sql(table_name: &str, ordered_indices: &[usize]) -> String {
    if ordered_indices.len() == FIELD_NAMES.len() {
        // 全量快速路径：与现有完全一致
        return format!("INSERT INTO {table_name} VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)");
    }
    let cols: Vec<&str> = ordered_indices.iter().map(|&i| FIELD_NAMES[i]).collect();
    let placeholders = vec!["?"; ordered_indices.len()].join(", ");
    format!("INSERT INTO {table_name} ({}) VALUES ({placeholders})", cols.join(", "))
}
```

**`do_insert_preparsed()` 现有投影路径模式**（lines 158-186，构建全量 `[Value; 15]` 再按 bitmask 过滤）：
```rust
// 现有投影路径（lines 159-186）
let all: [Value; 15] = [ /* 15 个字段的 Value */ ];
let selected: Vec<Value> = all.into_iter().enumerate()
    .filter(|(i, _)| field_mask.is_active(*i))
    .map(|(_, v)| v)
    .collect();
stmt.execute(rusqlite::params_from_iter(selected))?;
```

**改后：按 `ordered_indices` 取元素（需 ref 不能 move，因 `all` 需要保持所有权供索引）：**
```rust
// 改后：all 数组构建不变，只改 selected 选取方式
let all: [Value; 15] = [ /* 保持不变 */ ];
let selected: Vec<&Value> = ordered_indices.iter().map(|&i| &all[i]).collect();
stmt.execute(rusqlite::params_from_iter(selected))?;
```

**`initialize()` 中调用点**（lines 240-246，需同步更新 `build_insert_sql` / `build_create_sql` 调用签名）：
```rust
// 现有（line 241）：
self.insert_sql = Self::build_insert_sql(&self.table_name, self.field_mask);
let create_sql = Self::build_create_sql(&self.table_name, self.field_mask);

// 改后：
self.insert_sql = Self::build_insert_sql(&self.table_name, &self.ordered_indices);
let create_sql = Self::build_create_sql(&self.table_name, &self.ordered_indices);
```

**测试模式**（参考同文件 `test_sqlite_basic_export`，lines 346-375）：
```rust
#[test]
fn test_sqlite_field_order() {
    // ordered_indices = [10, 4]（sql, username）
    // 验证 CREATE TABLE 只有两列且顺序正确
    // 验证插入数据可正常查询
}
```

---

### `src/cli/run.rs` — 并行路径传递 `ordered_indices`

**Analog（同文件）:** `process_csv_parallel` 函数签名 + `field_mask` 传递模式（lines 424-436, 508-510, 675-688）

**现有 `field_mask` 在并行路径中的传递模式**（lines 435, 510, 640, 686）：

```rust
// 函数签名（line 435）：field_mask: FieldMask（Copy 类型，直接传值）
fn process_csv_parallel(
    // ...
    field_mask: FieldMask,
    // ...
) -> Result<(Vec<(PathBuf, usize)>, usize)>

// 内部使用（line 510）：
exporter.field_mask = field_mask;

// handle_run 计算（line 640）：
let field_mask = final_cfg.features.field_mask();

// handle_run 传入（line 686）：
field_mask,
```

**新增 `ordered_indices` 传递，在每处 `field_mask` 旁边同步添加：**
```rust
// 函数签名新增参数（紧跟 field_mask 之后）：
fn process_csv_parallel(
    // ...
    field_mask: FieldMask,
    ordered_indices: &[usize],          // 新增
    // ...
) -> Result<(Vec<(PathBuf, usize)>, usize)>

// 内部使用（紧跟 exporter.field_mask 之后）：
exporter.field_mask = field_mask;
exporter.ordered_indices = ordered_indices.to_vec();  // 新增，to_vec() 给每个并行任务独立所有权

// handle_run 计算（紧跟 field_mask 之后）：
let field_mask = final_cfg.features.field_mask();
let ordered_indices = final_cfg.features.ordered_field_indices();  // 新增

// handle_run 传入（紧跟 field_mask, 之后）：
field_mask,
&ordered_indices,   // 新增
```

---

## Shared Patterns

### 字段声明约定
**来源:** `src/exporter/csv.rs` lines 31-33，`src/exporter/sqlite.rs` lines 17-19
**应用到:** `CsvExporter` 和 `SqliteExporter` 新增字段
```rust
// 新字段紧跟 field_mask 声明，可见性与 field_mask 保持一致
pub(crate) field_mask: crate::features::FieldMask,
pub(crate) ordered_indices: Vec<usize>,   // CsvExporter 用 pub(crate)
pub(super) field_mask: crate::features::FieldMask,
pub(super) ordered_indices: Vec<usize>,   // SqliteExporter 用 pub(super)
```

### 全量快速路径保留约定
**来源:** `src/exporter/csv.rs` line 98，`src/exporter/sqlite.rs` line 136
**应用到:** 两个 exporter 的热路径
```rust
// CSV：全量掩码快速路径判断不变（line 98）
if field_mask == crate::features::FieldMask::ALL {
    // 全量写入，不经过 ordered_indices 分发
}

// SQLite：同样保留全量 params! 快速路径（line 136）
if field_mask == crate::features::FieldMask::ALL {
    stmt.execute(params![...])?;
    return Ok(());
}
```

### `#[must_use]` 约定
**来源:** `src/features/mod.rs` line 130（`field_mask()` 方法声明）
**应用到:** `ordered_field_indices()` 方法
```rust
#[must_use]
pub fn ordered_field_indices(&self) -> Vec<usize> { ... }
```

### 测试辅助函数复用
**来源:** `src/exporter/csv.rs` lines 448-460（`write_test_log`），`src/exporter/sqlite.rs` lines 331-342（同名函数）
**应用到:** 新增的字段顺序集成测试，直接复用已有的 `write_test_log` helper

---

## No Analog Found

无。所有修改文件均有精确的同文件 analog，Phase 2 是纯扩展性修改。

---

## Metadata

**Analog search scope:** `src/features/`, `src/exporter/`, `src/cli/`
**Files scanned:** 5
**Pattern extraction date:** 2026-04-18

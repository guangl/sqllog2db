# Phase 12: SQL 模板归一化引擎 - Pattern Map

**Mapped:** 2026-05-15
**Files analyzed:** 4
**Analogs found:** 4 / 4

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/features/sql_fingerprint.rs` | utility (transform) | transform | `src/features/sql_fingerprint.rs` (fingerprint()) | exact — 同文件扩展 |
| `src/features/mod.rs` | config + pub-use | request-response | `src/features/mod.rs` (ReplaceParametersConfig) | exact — 同文件扩展 |
| `src/config.rs` | config | — | `src/config.rs` (FeaturesConfig re-export) | exact — 同文件，无需修改（FeaturesConfig 在 mod.rs） |
| `src/cli/run.rs` | orchestration (hot loop) | streaming | `src/cli/run.rs` (do_normalize 条件调用) | exact — 同文件扩展 |

---

## Pattern Assignments

### `src/features/sql_fingerprint.rs` — 新增 `normalize_template()` + 共享扫描基础

**Analog:** `src/features/sql_fingerprint.rs` — 现有 `fingerprint()` 函数

#### NEEDS_SPECIAL 字节表（lines 1–18）— 须扩展

```rust
const NEEDS_SPECIAL: [bool; 256] = {
    let mut t = [false; 256];
    t[b'\'' as usize] = true;
    t[b' ' as usize] = true;
    t[b'\t' as usize] = true;
    t[b'\n' as usize] = true;
    t[b'\r' as usize] = true;
    t[0x0B_usize] = true; // vertical tab
    t[0x0C_usize] = true; // form feed
    let mut d = b'0';
    while d <= b'9' {
        t[d as usize] = true;
        d += 1;
    }
    t
};
```

**注意（Pitfall 5）：** `normalize_template` 需要额外将 `b'-'`（0x2D）和 `b'/'`（0x2F）标记为 `true`，否则注释起始字节会被批量复制路径跳过，注释从未被检测到。有两种策略：
- 方案 A：新建 `NEEDS_SPECIAL_NORMALIZE: [bool; 256]` 常量（在 `-` 和 `/` 上额外置 `true`），`normalize_template` 使用此表，`fingerprint` 继续用原表 — **推荐**，零性能影响。
- 方案 B：将原 `NEEDS_SPECIAL` 扩展（会让 `fingerprint` 多分发两种字节），应避免。

#### 核心扫描循环结构（lines 34–87）— 复制并扩展

```rust
pub fn fingerprint(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut out: Vec<u8> = Vec::with_capacity(sql.len());
    let mut i = 0;

    while i < len {
        // 批量复制普通字节（字母、符号等），跳过逐字节分发开销
        let bulk_start = i;
        while i < len && !NEEDS_SPECIAL[bytes[i] as usize] {
            i += 1;
        }
        if i > bulk_start {
            out.extend_from_slice(&bytes[bulk_start..i]);
        }
        if i >= len {
            break;
        }

        match bytes[i] {
            b'\'' => {
                out.push(b'?');
                i += 1;
                // 用 memchr 跳到下一个引号，避免逐字节扫描
                loop {
                    let Some(rel) = memchr::memchr(b'\'', &bytes[i..]) else {
                        i = len;
                        break;
                    };
                    i += rel + 1;
                    if i < len && bytes[i] == b'\'' {
                        i += 1; // '' 转义，继续消费
                    } else {
                        break;
                    }
                }
            }
            b if b.is_ascii_digit() && !prev_is_ident_byte(&out) => {
                out.push(b'?');
                i += 1;
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
            }
            b if b.is_ascii_whitespace() => {
                if !matches!(out.last(), Some(&b' ')) {
                    out.push(b' ');
                }
                i += 1;
                while i < len && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }

    let out_str = String::from_utf8(out).expect("fingerprint: invalid UTF-8");
    let trimmed = out_str.trim_ascii();
    if trimmed.len() == out_str.len() {
        out_str
    } else {
        trimmed.to_string()
    }
}
```

**`normalize_template` 在此基础上的扩展点：**

1. **字符串字面量分支（`b'\''`）**：保留原文（不替换为 `?`），用 memchr 跳过字面量内容后原样写回。该分支必须先于注释检测（D-15）。
2. **注释去除 — 单行（新增分支）**：
   ```rust
   b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
       if let Some(rel) = memchr::memchr(b'\n', &bytes[i..]) {
           i += rel + 1;
       } else {
           i = len;
       }
       // 不向 out 写入任何内容（D-16）
   }
   ```
3. **注释去除 — 多行（新增分支）**：
   ```rust
   b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
       i += 2;
       if let Some(rel) = memchr::memmem::find(&bytes[i..], b"*/") {
           i += rel + 2;
       } else {
           i = len;
       }
       // 替换为单空格，避免两侧 token 粘连（D-16）
       if !matches!(out.last(), Some(&b' ')) {
           out.push(b' ');
       }
   }
   ```
4. **关键字大写化（新增分支）**：遇到字母字节时，读取完整单词（直到非 ident 字节），调用 `is_keyword()` 判断；是则大写输出，否则原样输出。必须做双向单词边界检查（Pitfall 2、4）。
5. **数字字面量**：`normalize_template` 中数字不替换为 `?`（数字原样保留，仅空白折叠）。
6. **IN 列表折叠（策略 A — 主循环内联）**：检测到独立关键字 `IN` 后，向前跳过空白，若下一个非空白字节是 `(`，则消费括号内全部内容，输出 `IN (?)`（D-01/D-02/D-03）。

#### `prev_is_ident_byte` 辅助函数（lines 106–109）— 直接复用

```rust
fn prev_is_ident_byte(out: &[u8]) -> bool {
    out.last()
        .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}
```

该函数在 `normalize_template` 中同样需要，用于关键字边界检测（前向边界：前一个输出字节是否为标识符字节）。

#### 测试模块结构（lines 112–162）— 仿照扩展

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_literal_replaced() {
        assert_eq!(fingerprint("WHERE name = 'alice'"), "WHERE name = ?");
    }
    // ... 同模式添加 normalize_template 测试
}
```

新增的 `normalize_template` 测试需覆盖 RESEARCH.md §Validation Architecture 中列出的所有 Req ID（注释去除、IN 折叠、关键字大写化、空白折叠、字面量内注释保护、相同语义不同参数产生相同 key）。

---

### `src/features/mod.rs` — 新增 `TemplateAnalysisConfig` + `pub use` 导出

**Analog:** `src/features/mod.rs` — `ReplaceParametersConfig` 结构体（lines 73–113）

#### `ReplaceParametersConfig` 完整模式（lines 73–113）— 直接仿照

```rust
/// `[features.replace_parameters]` 配置段
#[derive(Debug, Deserialize, Clone)]
pub struct ReplaceParametersConfig {
    #[serde(default = "default_true")]
    pub enable: bool,
    #[serde(default)]
    pub placeholders: Vec<String>,
}

impl Default for ReplaceParametersConfig {
    fn default() -> Self {
        Self {
            enable: true,
            placeholders: Vec::new(),
        }
    }
}
```

**`TemplateAnalysisConfig` 仿照此结构**，差异点：
- 字段名为 `enabled`（D-12，与 `enable` 区分），默认 `false`（D-10，用 `#[serde(default)]` 即可，无需 `default_true`）
- 无额外字段（D-10）

```rust
/// `[features.template_analysis]` 配置段
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

#### `FeaturesConfig` 结构体（lines 120–126）— 新增字段

```rust
#[derive(Debug, Deserialize, Clone, Default)]
pub struct FeaturesConfig {
    pub filters: Option<FiltersFeature>,
    pub replace_parameters: Option<ReplaceParametersConfig>,
    pub fields: Option<Vec<String>>,
    // 新增（D-11）：
    pub template_analysis: Option<TemplateAnalysisConfig>,
}
```

#### `pub use` 导出模式（lines 7–8）— 仿照新增

```rust
pub mod sql_fingerprint;
pub use sql_fingerprint::fingerprint;
// 新增（D-09）：
pub use sql_fingerprint::normalize_template;
```

#### 测试模式（lines 274–279）— 仿照扩展

```rust
#[test]
fn test_replace_parameters_config_default() {
    let cfg = ReplaceParametersConfig::default();
    assert!(cfg.enable);
    assert!(cfg.placeholders.is_empty());
}
```

新增对应测试：`test_template_analysis_config_default`（验证 `enabled` 默认为 `false`）、`test_features_config_default`（验证 `template_analysis` 字段为 `None`）。

---

### `src/cli/run.rs` — 热循环中条件调用 `normalize_template()`

**Analog:** `src/cli/run.rs` — `do_normalize` 变量声明（lines 697–703）及热循环内的条件调用（lines 206–219）

#### `do_normalize` 计算模式（lines 697–703）— 直接仿照

```rust
// 如果字段投影排除了 normalized_sql（字段 14），则禁用参数替换计算
let do_normalize = field_mask.includes_normalized_sql()
    && final_cfg
        .features
        .replace_parameters
        .as_ref()
        .is_none_or(|r| r.enable);
```

**`do_template` 仿照此模式**（D-13），在同一位置声明（`do_normalize` 之后）：

```rust
let do_template = final_cfg
    .features
    .template_analysis
    .as_ref()
    .is_some_and(|t| t.enabled);
```

区别：`do_template` 不依赖 `field_mask`（模板 key 不占用现有字段列），且 `TemplateAnalysisConfig` 只有 `Option` 存在时才启用（`is_some_and` vs `is_none_or`）。

#### 热循环内条件调用模式（lines 206–219）— 直接仿照

```rust
let ns = if do_normalize
    && (!params_buffer.is_empty() || record.tag.is_none())
{
    crate::features::compute_normalized(
        &record,
        &meta,
        pm.sql.as_ref(),
        params_buffer,
        placeholder_override,
        ns_scratch,
    )
} else {
    None
};
```

**`normalize_template` 调用仿照此模式**（D-13/D-14），在 `ns` 赋值之后添加：

```rust
let tmpl_key: Option<String> = if do_template {
    Some(crate::features::normalize_template(pm.sql.as_ref()))
} else {
    None
};
// tmpl_key 暂存为局部变量，Phase 13 的 TemplateAggregator::observe() 将消费此值（D-14）
```

**零开销保证：** `do_template = false` 时分支不执行，`normalize_template` 完全不调用。

#### `process_log_file` 函数签名（lines 114–129）— 新增参数

```rust
fn process_log_file(
    file_path: &str,
    file_index: usize,
    total_files: usize,
    exporter_manager: &mut ExporterManager,
    pipeline: &Pipeline,
    pb: &ProgressBar,
    limit: Option<usize>,
    interrupted: &Arc<AtomicBool>,
    do_normalize: bool,
    placeholder_override: Option<bool>,
    params_buffer: &mut ParamBuffer,
    ns_scratch: &mut Vec<u8>,
    reset_pb: bool,
    sql_record_filter: Option<&CompiledSqlFilters>,
) -> Result<usize>
```

新增 `do_template: bool` 参数，位置参照 `do_normalize: bool`（紧跟其后），所有调用点同步更新。

---

## Shared Patterns

### 空白折叠
**Source:** `src/features/sql_fingerprint.rs` lines 72–80
**Apply to:** `normalize_template()` 继承此完整逻辑

```rust
b if b.is_ascii_whitespace() => {
    if !matches!(out.last(), Some(&b' ')) {
        out.push(b' ');
    }
    i += 1;
    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
}
```

### UTF-8 安全输出
**Source:** `src/features/sql_fingerprint.rs` lines 89–102
**Apply to:** `normalize_template()` 使用相同的 `String::from_utf8(out).expect(...)` + `trim_ascii()` 模式

```rust
let out_str = String::from_utf8(out).expect("normalize_template: invalid UTF-8");
let trimmed = out_str.trim_ascii();
if trimmed.len() == out_str.len() {
    out_str
} else {
    trimmed.to_string()
}
```

**安全性论证（与 fingerprint 相同）：** `normalize_template` 只操作 ASCII 字节（大写化关键字、注释去除、空白折叠），不拆断多字节 UTF-8 序列（>= 0x80 字节始终在批量复制路径中原样输出）。

### memchr 字符串字面量跳过
**Source:** `src/features/sql_fingerprint.rs` lines 48–64 / `src/features/replace_parameters.rs` lines 82–93
**Apply to:** `normalize_template()` 字符串字面量原文保留分支（D-15）

```rust
b'\'' => {
    // normalize_template: 保留原文（不替换为 ?）
    let literal_start = i;
    i += 1;
    loop {
        let Some(rel) = memchr::memchr(b'\'', &bytes[i..]) else {
            i = len;
            break;
        };
        i += rel + 1;
        if i < len && bytes[i] == b'\'' {
            i += 1; // '' 转义
        } else {
            break;
        }
    }
    out.extend_from_slice(&bytes[literal_start..i]);
}
```

### serde Default 模式
**Source:** `src/features/mod.rs` lines 87–94（`ReplaceParametersConfig::Default impl`）
**Apply to:** `TemplateAnalysisConfig::Default impl`

```rust
impl Default for ReplaceParametersConfig {
    fn default() -> Self {
        Self {
            enable: true,
            placeholders: Vec::new(),
        }
    }
}
```

### `is_some_and` / `is_none_or` Option 习惯用法
**Source:** `src/cli/run.rs` lines 698–703
**Apply to:** `do_template` 的计算，以及 `FeaturesConfig` 中访问 `template_analysis` 的任何地方

---

## No Analog Found

本 phase 所有 4 个文件在代码库中均有直接类比，无需回退到 RESEARCH.md 模式。

---

## Metadata

**Analog search scope:** `src/features/`, `src/cli/`, `src/config.rs`
**Files scanned:** 4（sql_fingerprint.rs, mod.rs, replace_parameters.rs, run.rs）
**Pattern extraction date:** 2026-05-15

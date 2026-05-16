# Phase 15: SVG 图表基础设施 + 前两类图表 - Pattern Map

**Mapped:** 2026-05-16
**Files analyzed:** 9 (3 new, 6 modified)
**Analogs found:** 9 / 9

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `src/charts/mod.rs` | module-entry / utility | file-I/O | `src/exporter/mod.rs` | role-match |
| `src/charts/frequency_bar.rs` | utility (chart renderer) | file-I/O | `src/exporter/csv.rs` (`write_companion_rows`) | partial |
| `src/charts/latency_hist.rs` | utility (chart renderer) | file-I/O | `src/exporter/csv.rs` (`write_companion_rows`) | partial |
| `src/features/mod.rs` | config / model | — | `src/features/mod.rs` itself (`ReplaceParametersConfig`, `TemplateAnalysisConfig`) | exact |
| `src/config.rs` | config validation | — | `src/config.rs` itself (fields validation block, L66-79) | exact |
| `src/features/template_aggregator.rs` | domain model | — | `src/features/template_aggregator.rs` itself (`finalize` sort + iterate) | exact |
| `src/cli/run.rs` | orchestration / CLI | request-response | `src/cli/run.rs` itself (L904-915 finalize sequence) | exact |
| `src/main.rs` | entrypoint | — | `src/main.rs` itself (mod declarations L6-15) | exact |
| `Cargo.toml` | config | — | `Cargo.toml` itself (existing optional-feature deps) | exact |

---

## Pattern Assignments

### `src/charts/mod.rs` (module entry, file-I/O)

**Analog:** `src/exporter/mod.rs` — 公共入口模块，声明子模块，暴露统一接口函数

**Submodule declaration pattern** (`src/exporter/mod.rs` lines 6-9):
```rust
pub mod csv;
pub mod sqlite;
pub use csv::CsvExporter;
pub use sqlite::SqliteExporter;
```
新模块照此：
```rust
pub mod frequency_bar;
pub mod latency_hist;
```

**Directory creation pattern** — `ensure_parent_dir` 定义为 `pub(super)`（`src/exporter/mod.rs` 中），
**`src/charts/` 不能访问它**，必须内联等价逻辑：
```rust
// src/charts/mod.rs — 直接调用标准库，与 ensure_parent_dir 语义等价
std::fs::create_dir_all(output_dir).map_err(|e| {
    crate::error::Error::File(crate::error::FileError::CreateDirectoryFailed {
        path: output_dir.to_path_buf(),
        reason: e.to_string(),
    })
})?;
```

**Error mapping pattern** — 参照 `src/exporter/csv.rs` 的 `write_companion_rows`（使用 `crate::error::Result`）和 `src/exporter/mod.rs` 内部错误构造方式。`src/charts/` 模块返回类型使用 `crate::error::Result<()>`。

**`generate_charts` 入口结构**（参照 CONTEXT.md L113-119 的调用点）：
```rust
pub fn generate_charts(agg: &TemplateAggregator, cfg: &ChartsConfig) -> Result<()> {
    let output_dir = std::path::Path::new(&cfg.output_dir);
    std::fs::create_dir_all(output_dir).map_err(|e| { /* error转换 */ })?;

    let entries: Vec<_> = agg.iter_chart_entries().collect();

    if cfg.frequency_bar {
        let path = output_dir.join("top_n_frequency.svg");
        frequency_bar::draw_frequency_bar(&entries, cfg.top_n, &path)?;
    }
    if cfg.latency_hist {
        for entry in entries.iter().take(cfg.top_n) {
            let filename = format!("latency_histogram_{}.svg", sanitize_filename(entry.key));
            let path = output_dir.join(&filename);
            latency_hist::draw_latency_hist(entry.key, entry.histogram, &path)?;
        }
    }
    Ok(())
}
```

**sanitize_filename 辅助函数**（D-02, Specifics §文件名 sanitize）：
```rust
fn sanitize_filename(key: &str) -> String {
    let sanitized: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    sanitized.chars().take(80).collect()
}
```

---

### `src/charts/frequency_bar.rs` (chart renderer, file-I/O)

**Analog:** `src/exporter/csv.rs` — `write_companion_rows` / `format_companion_row`（写文件、flush 显式调用、子函数拆分）

**Flush pattern** (`src/exporter/csv.rs` flush 模式，类比 `root.present()`):
```rust
// csv.rs 中显式 flush
writer.flush()?;
// charts 中 SVGBackend 的等价操作（SC-4）
root.present()?;
```

**函数拆分模式**（CLAUDE.md：函数 ≤ 40 行）— `csv.rs` 将 `format_companion_row` 拆为独立函数；
`frequency_bar.rs` 同理：`draw_frequency_bar` 调用 `truncate_label` 子函数。

**plotters SVGBackend + ChartBuilder 骨架**（来自 RESEARCH.md Pattern 1/2）：
```rust
use plotters::prelude::*;

pub fn draw_frequency_bar(
    entries: &[ChartEntry<'_>],
    top_n: usize,
    output_path: &std::path::Path,
) -> crate::error::Result<()> {
    let data: Vec<(String, u64)> = entries
        .iter()
        .take(top_n)
        .map(|e| (truncate_label(e.key, 40), e.count))
        .collect();

    let max_count = data.iter().map(|(_, c)| *c).max().unwrap_or(1);
    let n = data.len();

    let root = SVGBackend::new(output_path, (1200, 600)).into_drawing_area();
    root.fill(&WHITE).map_err(/* error 转换 */)?;
    // ... ChartBuilder、configure_mesh、draw_series ...
    root.present().map_err(/* error 转换 */)?;  // SC-4 必须显式调用
    Ok(())
}
```

**Y 轴 SegmentedCoord 标签映射**（RESEARCH.md Pitfall 5 + Pattern 2）：
```rust
let labels: Vec<String> = data.iter().map(|(k, _)| k.clone()).collect();
chart.configure_mesh()
    .y_label_formatter(&|v: &SegmentValue<usize>| match v {
        SegmentValue::CenterOf(i) | SegmentValue::Exact(i) =>
            labels.get(*i).cloned().unwrap_or_default(),
        SegmentValue::Last => String::new(),
    })
    .x_desc("Execution Count")
    .draw()?;

chart.draw_series(
    Histogram::horizontal(&chart)
        .style(RGBColor(70, 130, 180).filled())  // steelblue D-09 自行决定
        .margin(5)
        .data(data.iter().enumerate().map(|(i, (_, count))| (i, *count))),
)?;
```

**truncate_label 子函数**（D-08，使用 `.chars()` 保证 Unicode 安全）：
```rust
fn truncate_label(key: &str, max_chars: usize) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= max_chars {
        key.to_string()
    } else {
        let truncated: String = chars[..max_chars - 1].iter().collect();
        format!("{}…", truncated)
    }
}
```

---

### `src/charts/latency_hist.rs` (chart renderer, file-I/O)

**Analog:** `src/exporter/csv.rs` — 写文件、flush、子函数拆分（同上）

**hdrhistogram iter_recorded() 迭代模式**（RESEARCH.md Pattern 3）：
```rust
let buckets: Vec<(u64, u64)> = histogram
    .iter_recorded()
    .map(|v| (v.value_iterated_to(), v.count_at_value()))
    .collect();
```

**对数 X 轴（log_scale() 而非 LogRange）**（RESEARCH.md State of the Art）：
```rust
// 正确：
.build_cartesian_2d((min_val..max_val).log_scale(), 0u64..max_count)?
// 禁止（deprecated）：
// LogRange::new(min_val, max_val)
```

**手动 Rectangle 绘制预计算 bucket**（RESEARCH.md Pitfall 3 — 不用 Histogram series）：
```rust
chart.draw_series(
    buckets.windows(2).map(|pair| {
        let (left, _) = pair[0];
        let (right, count) = pair[1];
        Rectangle::new(
            [(left, 0u64), (right, count)],
            RGBColor(70, 130, 180).filled(),
        )
    }),
)?;
root.present()?;  // SC-4
```

**空 bucket 防御**（RESEARCH.md Pitfall 2）：
```rust
if buckets.is_empty() {
    return Ok(());
}
let min_val = buckets.first().map(|(v, _)| (*v).max(1)).unwrap_or(1);
```

---

### `src/features/mod.rs` — 新增 `ChartsConfig` + `charts` 字段 (config, model)

**Analog:** `src/features/mod.rs` 自身 — `ReplaceParametersConfig`（L79-118）、`TemplateAnalysisConfig`（L124-130）、`FeaturesConfig`（L133-140）

**`Option<XxxConfig>` 字段模式**（`FeaturesConfig` L135-139）：
```rust
pub struct FeaturesConfig {
    pub filters: Option<FiltersFeature>,
    pub replace_parameters: Option<ReplaceParametersConfig>,
    pub fields: Option<Vec<String>>,
    pub template_analysis: Option<TemplateAnalysisConfig>,
    // Phase 15 新增（与上面完全对称）：
    pub charts: Option<ChartsConfig>,
}
```

**Config struct with serde defaults 模式**（`ReplaceParametersConfig` L79-99）：
```rust
#[derive(Debug, Deserialize, Clone)]
pub struct ChartsConfig {
    pub output_dir: String,
    #[serde(default = "default_top_n")]
    pub top_n: usize,
    #[serde(default = "default_true")]  // 复用已有 default_true()（L120-122）
    pub frequency_bar: bool,
    #[serde(default = "default_true")]
    pub latency_hist: bool,
}

fn default_top_n() -> usize { 10 }
// default_true() 已在 mod.rs L120-122 定义，直接复用，不重复定义
```

**pub use 导出模式**（L1-13）：
```rust
// 在 mod.rs 顶部与其他 pub use 并列：
pub use template_aggregator::ChartEntry;
```

**Test 模式**（L296-308，serde 反序列化单元测试）：
```rust
#[test]
fn test_charts_config_deserialize_defaults() {
    let cfg: ChartsConfig = toml::from_str(r#"output_dir = "charts/""#).unwrap();
    assert_eq!(cfg.top_n, 10);
    assert!(cfg.frequency_bar);
    assert!(cfg.latency_hist);
}
```

---

### `src/config.rs` — 新增 charts 验证 (config validation)

**Analog:** `src/config.rs` 自身 — `validate()` 中 `features.fields` 字段名验证块（L66-79）

**跨字段依赖验证模式**（`validate()` L58-79，`ConfigError::InvalidValue` 构造方式）：
```rust
// 已有模式（L66-79）：
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

// Phase 15 新增（D-06，模式完全对称）：
if self.features.charts.is_some() {
    let ta_enabled = self.features
        .template_analysis
        .as_ref()
        .is_some_and(|ta| ta.enabled);
    if !ta_enabled {
        return Err(Error::Config(ConfigError::InvalidValue {
            field: "features.charts".to_string(),
            value: String::new(),
            reason: "启用 [features.charts] 需要先设置 [features.template_analysis]\nenabled = true"
                .to_string(),
        }));
    }
}
```

**`validate_and_compile()` 也需要同样的检查**（L97-131）— 两个方法均须添加，
保持与现有 `fields` 验证同步（两个方法都有 fields 检查块）。

**`apply_one()` 新增 charts keys 模式**（L149-265，`get_or_insert_with(Default::default)`）：
```rust
// 已有模式（L255-260）：
"features.template_analysis.enabled" => {
    self.features
        .template_analysis
        .get_or_insert_with(Default::default)
        .enabled = parse_bool(value)?;
}
// Phase 15 类比（需要 ChartsConfig 实现 Default）：
"features.charts.output_dir" => {
    self.features
        .charts
        .get_or_insert_with(Default::default)
        .output_dir = value.to_string();
}
"features.charts.top_n" => { /* parse usize */ }
"features.charts.frequency_bar" => { /* parse_bool */ }
"features.charts.latency_hist" => { /* parse_bool */ }
```

注意：`ChartsConfig` 需要实现 `Default`（因 `get_or_insert_with(Default::default)` 调用），
`output_dir` 无合理默认值，可用空字符串或 `"charts/"` 作为默认。

---

### `src/features/template_aggregator.rs` — 新增 `ChartEntry` + `iter_chart_entries()` (domain model)

**Analog:** `src/features/template_aggregator.rs` 自身 — `finalize()` 方法（L102-130，排序 + 迭代 + 字段访问）

**`finalize()` 的排序逻辑**（L124-129）是 `iter_chart_entries()` 的直接类比：
```rust
// 已有：finalize 中的排序（L124-129）
stats.sort_unstable_by(|a, b| {
    b.count.cmp(&a.count).then_with(|| a.template_key.cmp(&b.template_key))
});

// 新增：iter_chart_entries 中的排序（与 finalize 一致，但不消耗 self）
let mut entries: Vec<ChartEntry<'_>> = self.entries
    .iter()
    .map(|(key, entry)| ChartEntry {
        key: key.as_str(),
        count: entry.histogram.len(),
        histogram: &entry.histogram,
    })
    .collect();
entries.sort_unstable_by(|a, b| {
    b.count.cmp(&a.count).then_with(|| a.key.cmp(b.key))
});
entries.into_iter()
```

**`ChartEntry` pub struct 定义**（参照 `TemplateStats` L25-37 的公共字段模式）：
```rust
/// 图表生成专用的只读视图（在 finalize 前调用，不消耗 self）
pub struct ChartEntry<'a> {
    pub key: &'a str,
    pub count: u64,
    pub histogram: &'a hdrhistogram::Histogram<u64>,
}
```

**关键约束：** `histogram` 字段当前为私有（`struct TemplateEntry` 是私有结构体，`histogram` 字段私有），
`iter_chart_entries` 必须定义在 `template_aggregator.rs` 内部，才能访问私有字段（Rust 可见性规则）。

**Test 模式**（L138-145，单观测后验证）：
```rust
#[test]
fn test_iter_chart_entries_count() {
    let mut agg = TemplateAggregator::new();
    agg.observe("SELECT 1", 100, "2025-01-15 10:00:00");
    agg.observe("SELECT 1", 200, "2025-01-15 10:00:01");
    let entries: Vec<_> = agg.iter_chart_entries().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].count, 2);
}
```

---

### `src/cli/run.rs` — 插入 `generate_charts` 调用点 (orchestration)

**Analog:** `src/cli/run.rs` 自身 — L904-915（`exporter_manager.finalize()` → `template_agg.map(finalize)` 序列）

**当前顺序路径代码**（L905-915）：
```rust
exporter_manager.finalize()?;
if !quiet {
    exporter_manager.log_stats();
}

let template_stats = template_agg.map(TemplateAggregator::finalize);
if let Some(ref stats) = template_stats {
    info!("Template analysis: {} unique templates", stats.len());
    exporter_manager.write_template_stats(stats, None)?;
}
```

**Phase 15 修改后的顺序路径**（在 `exporter_manager.finalize()` 之前插入，CONTEXT.md L113-119）：
```rust
// 在 exporter_manager.finalize() 之前插入：
if let Some(ref agg) = template_agg {
    if let Some(charts_cfg) = cfg.features.charts.as_ref() {
        crate::charts::generate_charts(agg, charts_cfg)?;
    }
}

exporter_manager.finalize()?;
// ... 后续不变 ...
```

**并行路径插入点**（L804-814，`parallel_agg.map(TemplateAggregator::finalize)` 之前）：
```rust
// 当前并行路径（L804-805）：
let template_stats = parallel_agg.map(TemplateAggregator::finalize);

// 修改：在 map/finalize 之前插入图表生成
if let Some(ref agg) = parallel_agg {
    if let Some(charts_cfg) = final_cfg.features.charts.as_ref() {
        crate::charts::generate_charts(agg, charts_cfg)?;
    }
}
let template_stats = parallel_agg.map(TemplateAggregator::finalize);
```

**`if let Some(ref x) = option` 模式**（run.rs L806、L912 中大量使用）是项目一贯风格。

---

### `src/main.rs` — 新增 `mod charts;` (entrypoint)

**Analog:** `src/main.rs` 自身 — L6-15 的 mod 声明块

**已有 mod 声明块**（L6-15）：
```rust
mod cli;
mod color;
mod config;
mod error;
mod exporter;
mod features;
mod lang;
mod logging;
mod parser;
mod resume;
```

**新增位置**：按字母序插入，在 `mod cli;` 之后：
```rust
mod charts;
mod cli;
// ...
```

或者按功能分组放在 `mod exporter;` 附近（与输出相关）。按字母序是项目现有惯例（`cli/color/config/error/exporter/features/lang/logging/parser/resume` 大致字母序）。

---

### `Cargo.toml` — 新增 plotters 依赖 (config)

**Analog:** `Cargo.toml` 自身 — 已有条件 feature 依赖（`self_update` L44-48、`rusqlite` L65-69）

**已有 feature-limited 依赖模式**（L44-48）：
```toml
self_update = { version = "0.44.0", default-features = false, features = [
  "reqwest",
  "rustls",
  "compression-flate2",
] }
```

**Phase 15 新增**（RESEARCH.md Standard Stack + Claude 自行决定 feature flags）：
```toml
plotters = { version = "0.3.7", default-features = false, features = [
  "svg_backend",
  "all_series",
  "all_elements",
] }
```

注意：planner 应在 Wave 0 验证这三个 feature 组合是否足以编译（RESEARCH.md Assumption A2）。

---

## Shared Patterns

### 错误类型转换
**Source:** `src/exporter/csv.rs` + `src/error.rs`
**Apply to:** `src/charts/mod.rs`、`src/charts/frequency_bar.rs`、`src/charts/latency_hist.rs`

plotters 的错误类型为 `DrawingAreaErrorKind<SVGBackend>` 或 `Box<dyn std::error::Error>`，
需要转换为 `crate::error::Result<()>`。项目未定义 chart 专用错误变体，
最简洁方式是用 `map_err` 将 plotters 错误包装为现有变体，或为 `error.rs` 新增
`Error::Chart(String)` 变体。参照 `ExportError::IoError` 的封装方式。

### 函数长度约束
**Source:** CLAUDE.md — "Keep functions under 40 lines — split if longer"
**Apply to:** 所有 `src/charts/` 函数

- `draw_frequency_bar` ≥ 40 行时拆分为 `build_chart_data`、`configure_mesh_with_labels`、`draw_bars` 子函数
- `draw_latency_hist` 同理
- `generate_charts` 本身较短，但内部循环逻辑可拆为 `draw_all_latency_hists` 子函数

### `default_true()` 复用
**Source:** `src/features/mod.rs` L120-122
**Apply to:** `src/features/mod.rs` — `ChartsConfig` 的 `frequency_bar`/`latency_hist` 字段

`default_true()` 函数已在 `features/mod.rs` 定义，`ChartsConfig` 的 serde default 直接引用，不重复定义。

### `#[serde(default = "fn")]` 模式
**Source:** `src/config.rs` L299-300、`src/features/mod.rs` L88-89
**Apply to:** `ChartsConfig` 所有有默认值的字段

所有 `top_n = 10`、`frequency_bar = true`、`latency_hist = true` 均用 `#[serde(default = "fn_name")]`，
与 `ReplaceParametersConfig.enable` 的写法完全一致。

---

## No Analog Found

所有文件均能在现有代码库中找到合理类比，无需完全依赖 RESEARCH.md 模式的文件。

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `src/charts/frequency_bar.rs` (plotters API 部分) | chart renderer | file-I/O | plotters SVG API 是项目新引入库，核心绘图调用无现有类比，须参照 RESEARCH.md Pattern 2 |
| `src/charts/latency_hist.rs` (log_scale 部分) | chart renderer | file-I/O | 对数坐标轴是项目新模式，须参照 RESEARCH.md Pattern 3 |

---

## Metadata

**Analog search scope:** `src/features/`, `src/config.rs`, `src/main.rs`, `src/cli/run.rs`, `src/exporter/`, `Cargo.toml`
**Files scanned:** 8 source files + Cargo.toml
**Pattern extraction date:** 2026-05-16

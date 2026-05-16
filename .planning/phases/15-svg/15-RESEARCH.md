# Phase 15: SVG 图表基础设施 + 前两类图表 - Research

**Researched:** 2026-05-16
**Domain:** Rust plotters SVG 图表 + hdrhistogram 迭代器
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** 图表生成在 `TemplateAggregator::finalize()` 之前发生，直接访问 `TemplateAggregator` 内部原始 `Histogram<u64>`
- **D-02:** 每个 Top N 模板各自一张耗时分布直方图，文件名 `latency_histogram_<sanitized_key>.svg`
- **D-03:** 耗时直方图 X 轴使用对数刻度
- **D-04:** 独立 `[features.charts]` 配置段，`ChartsConfig` 新增至 `FeaturesConfig` 作为 `charts: Option<ChartsConfig>`
- **D-05:** ChartsConfig 字段：`output_dir`, `top_n=10`, `frequency_bar=true`, `latency_hist=true`
- **D-06:** `Config::validate()` 中：若 `features.charts` 存在，则 `features.template_analysis.enabled` 必须为 `true`
- **D-07:** `top_n_frequency.svg` 为横向条形图，Y 轴为模板 key，X 轴为执行次数，按频率降序排列
- **D-08:** Y 轴标签截断超过 40 字符时截断并追加 `"…"`
- **D-09:** 图表尺寸：1200×600 像素
- **D-10:** X 轴单位为执行次数（count），不转换为百分比
- **D-11:** 图表代码放在新模块 `src/charts/`：`mod.rs` / `frequency_bar.rs` / `latency_hist.rs`
- **D-12:** `run.rs` 单一调用点：`generate_charts(&agg, charts_cfg)?`
- **D-13:** `src/main.rs` 新增 `mod charts;`

### Claude's Discretion

- plotters 版本（选最新稳定版，写入 Cargo.toml 时检查 crates.io）
- plotters feature flags（svg backend only，禁用 bitmap_backend）
- 文件名 sanitize 逻辑细节（非 ASCII/非数字字符替换为 `_`，80 字符上限）
- 条形图颜色主题（单色 steelblue 系）
- 图表标题格式（包含运行时 N 值）
- latency_hist.svg 的图表尺寸

### Deferred Ideas (OUT OF SCOPE)

- 时间趋势折线图（`frequency_trend.svg`）— Phase 16
- 用户/Schema 占比饼图（`user_schema_pie.svg`）— Phase 16
- 可配置图表宽高 — 当前固定 1200×600
- 独立 JSON/CSV 统计报告（TMPL-03/03b）— Future v1.4+
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CHART-01 | 用户可在 config 的 `[features.charts]` 中指定 `output_dir`，启用后 run 结束时自动生成 SVG 文件到该目录 | ChartsConfig 结构设计、目录创建、generate_charts() 调用点 |
| CHART-02 | 生成 Top N 模板执行频率横向条形图（SVG），N 可在 config 中配置 | plotters Histogram::horizontal API、TemplateAggregator 公开访问接口 |
| CHART-03 | 生成全局耗时分布直方图（SVG），使用 hdrhistogram bucket 数据 | hdrhistogram iter_recorded() API、plotters log scale X 轴 |
</phase_requirements>

## Summary

Phase 15 在现有 `TemplateAggregator` 之上新增 `src/charts/` 模块，引入 `plotters` crate（SVG-only 配置）生成两类图表。关键技术挑战有三：一是 `TemplateAggregator` 内部 `TemplateEntry.histogram` 字段当前为私有，需要暴露 chart-oriented 迭代接口；二是 plotters `Histogram::horizontal` 所需的坐标系配置（Y 轴 `into_segmented()` + X 轴 count 范围）；三是耗时直方图的对数 X 轴，需用 `(min_us..max_us).log_scale()` 语法。

`hdrhistogram` 已在 Cargo.toml 中锁定为 `7.5.4`（项目已使用），`iter_recorded()` 返回仅包含非空 bucket 的迭代器，每项通过 `v.value_iterated_to()` / `v.count_at_value()` 获取值和计数。plotters 当前最新稳定版为 `0.3.7`，SVG-only 配置为 `default-features = false, features = ["svg_backend", "all_series"]`。

`ensure_parent_dir()` 在 `src/exporter/mod.rs` 中定义为 `pub(super)`，不能跨 crate 模块直接调用，需在 `src/charts/mod.rs` 内联等价逻辑（`std::fs::create_dir_all`）。

**Primary recommendation:** 先在 `TemplateAggregator` 新增 `pub fn iter_entries() -> impl Iterator<Item = ChartEntry<'_>>` 方法暴露每个 key 的 (count, histogram ref)，再用 plotters `Histogram::horizontal` + `into_segmented()` 绘制频率条形图，用手动 `Rectangle` 序列绘制对数刻度耗时直方图。

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| ChartsConfig 解析与验证 | Config 层（src/config.rs + src/features/mod.rs） | — | 与其他 FeaturesConfig 字段对称 |
| TemplateAggregator 数据暴露 | Domain 层（src/features/template_aggregator.rs） | — | histogram 私有字段，必须由 owner 暴露 |
| SVG 图表生成 | Chart 层（src/charts/） | — | 新模块，单一职责 |
| run.rs 调用点 | CLI 层（src/cli/run.rs） | — | 维持单一调用点 D-12 |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| plotters | 0.3.7 | SVG 图表生成 | Rust 生态事实标准图表库，159M 下载，官方 SVG backend | [VERIFIED: crates.io] |
| hdrhistogram | 7.5.4 | 耗时 histogram bucket 迭代 | 项目已依赖，锁定版本 | [VERIFIED: crates.io] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| itoa | 1.0（已有） | SVG 文本中整数格式化 | 写 count 标签时 | [VERIFIED: crates.io] |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| plotters | charts-rs | charts-rs 依赖字体系统，与 STATE.md 锁定决策冲突 |
| plotters | charming | charming 需要 JS 渲染器，不生成静态 SVG |
| plotters `Histogram::horizontal` | 手动 Rectangle | Histogram::horizontal 内置聚合，适合频率条形图；直方图桶需手动 Rectangle（因为坐标是预计算的 u64，不适合 segmented） |

**Cargo.toml 新增依赖：**
```toml
plotters = { version = "0.3.7", default-features = false, features = [
  "svg_backend",
  "all_series",
  "all_elements",
] }
```

**说明：**
- `svg_backend` — SVGBackend（必选）
- `all_series` — Histogram series（包含 Histogram::horizontal）
- `all_elements` — Rectangle 元素（直方图手动绘制）
- 不含 `bitmap_backend`、`ttf`、`chrono` — 避免字体/图像系统依赖（D-03 决策）

## Package Legitimacy Audit

slopcheck 在当前环境不可用，根据 crates.io 数据手动审核：

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| plotters 0.3.7 | crates.io | ~6 yrs | 159M total | github.com/plotters-rs/plotters | N/A (slopcheck unavailable) | [ASSUMED] Approved — 官方维护，Criterion.rs 使用 |
| hdrhistogram 7.5.4 | crates.io | ~7 yrs | 86M total | github.com/HdrHistogram/HdrHistogram_rust | N/A | 已在项目 Cargo.toml 中 |

**Packages removed due to slopcheck [SLOP] verdict:** 无
**Packages flagged as suspicious [SUS]:** 无（plotters 被广泛使用于 Rust 生态，Criterion 依赖）

*slopcheck 在研究时不可用，plotters 标记为 `[ASSUMED]`。鉴于其 159M 下载量和 Criterion.rs 等知名项目依赖，风险极低，但 planner 可选择性添加 checkpoint 验证。*

## Architecture Patterns

### System Architecture Diagram

```
handle_run()
    │
    ├── [顺序路径]
    │   ├── process_log_file() × N  ──→ template_agg.observe()
    │   ├── generate_charts(&agg, charts_cfg)?    ← Phase 15 新增
    │   │       ├── frequency_bar::draw(agg, cfg)  → top_n_frequency.svg
    │   │       └── latency_hist::draw(agg, cfg)   → latency_histogram_<key>.svg × top_n
    │   ├── exporter_manager.finalize()
    │   └── template_agg.finalize() → write_template_stats()
    │
    └── [并行路径]
        ├── process_csv_parallel() ──→ parallel_agg (merged)
        ├── generate_charts(&parallel_agg, charts_cfg)?  ← Phase 15 新增
        └── parallel_agg.finalize() → write_companion_rows()
```

### Recommended Project Structure
```
src/
├── charts/
│   ├── mod.rs           # pub fn generate_charts() + ChartEntry + sanitize_filename()
│   ├── frequency_bar.rs # pub fn draw_frequency_bar()
│   └── latency_hist.rs  # pub fn draw_latency_hist()
├── features/
│   ├── template_aggregator.rs  # 新增 pub fn iter_entries() + ChartEntry 结构体
│   └── mod.rs                  # 新增 ChartsConfig + pub use ChartEntry
└── config.rs            # validate() 新增 charts → template_analysis 依赖检查
```

### Pattern 1: plotters SVGBackend 基础骨架

**What:** 创建 SVG 文件的最小可运行模式
**When to use:** 每个 `draw_*` 函数的开头

```rust
// Source: docs.rs/plotters/0.3.7/plotters/backend/struct.SVGBackend.html
use plotters::prelude::*;

fn draw_chart(output_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let root = SVGBackend::new(output_path, (1200, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    // ... 绘制内容 ...

    root.present()?;  // 等价于 flush — SC-4 要求
    Ok(())
}
```

**关键：** `root.present()` 是 SVGBackend 的 flush 操作，必须显式调用（SC-4）。

### Pattern 2: 横向频率条形图（plotters Histogram::horizontal）

**What:** 使用 `plotters::series::Histogram::horizontal` 绘制横向条形图
**When to use:** `frequency_bar.rs` 中绘制 Top N 频率条形图

```rust
// Source: docs.rs/plotters/0.3.7/plotters/series/struct.Histogram.html
// 坐标系：X 轴 = count（u64），Y 轴 = 离散模板 key（segmented）

// 步骤 1：准备 Top N 数据（已按 count 降序，取前 top_n）
// entries: Vec<(&str, u64)>  →  (label, count)

let max_count = entries.iter().map(|(_, c)| *c).max().unwrap_or(1);
let n = entries.len();  // actual top_n

// 步骤 2：Y 轴使用 0..n 的离散范围，into_segmented() 为柱状图模式
let mut chart = ChartBuilder::on(&root)
    .caption(format!("Top {} SQL Templates by Frequency", n), ("sans-serif", 20))
    .margin(20)
    .x_label_area_size(40)
    .y_label_area_size(300)  // Y 轴标签区域需足够宽以容纳截断的模板 key
    .build_cartesian_2d(0u64..max_count, (0..n).into_segmented())?;

chart.configure_mesh()
    .y_label_formatter(&|v| {
        // SegmentedCoord 中 v 为 &SegmentValue<usize>，需映射到模板 key
        // 实际使用时通过 index 查找 entries[index].0
        String::new()  // 占位符，实际用 y_labels() 方法
    })
    .draw()?;

// 步骤 3：Histogram::horizontal — data 接收 (y_position, x_value) 元组
chart.draw_series(
    Histogram::horizontal(&chart)
        .style(RGBColor(70, 130, 180).filled())  // steelblue
        .margin(5)
        .data(entries.iter().enumerate().map(|(i, (_, count))| (i, *count))),
)?;
```

**注意：** `Histogram::horizontal` 的 `data()` 接收 `(discrete_y_pos, x_count)` 元组；`into_segmented()` 将整数范围转为离散分段坐标。

### Pattern 3: 对数刻度耗时直方图（手动 Rectangle + log scale）

**What:** hdrhistogram bucket → plotters 对数 X 轴直方图
**When to use:** `latency_hist.rs`

plotters 的 `Histogram` series 不适合预计算 bucket（它期望原始数据点并自行聚合）。对于 hdrhistogram 输出的 `(value, count)` bucket 对，使用**手动 Rectangle** 绘制：

```rust
// Source: hdrhistogram iter_recorded() + plotters manual Rectangle pattern

// 步骤 1：从 histogram 提取 bucket
// entry.histogram: &hdrhistogram::Histogram<u64>
let buckets: Vec<(u64, u64)> = histogram
    .iter_recorded()
    .map(|v| (v.value_iterated_to(), v.count_at_value()))
    .collect();

let min_val = buckets.first().map(|(v, _)| (*v).max(1)).unwrap_or(1);
let max_val = buckets.last().map(|(v, _)| *v).unwrap_or(1);
let max_count = buckets.iter().map(|(_, c)| *c).max().unwrap_or(1);

// 步骤 2：对数 X 轴（u64 实现 LogScalable）
// Source: github.com/plotters-rs/plotters logarithmic.rs — u64 implements LogScalable
let mut chart = ChartBuilder::on(&root)
    .caption(format!("Latency Distribution: {}", truncated_key), ("sans-serif", 18))
    .margin(20)
    .x_label_area_size(40)
    .y_label_area_size(60)
    .build_cartesian_2d(
        (min_val..max_val).log_scale(),  // 对数 X 轴（单位：µs）
        0u64..max_count,
    )?;

chart.configure_mesh().draw()?;

// 步骤 3：手动绘制每个 bucket 为 Rectangle
// 相邻 bucket 的左边界用前一个 value_iterated_to 确定
chart.draw_series(
    buckets.windows(2).map(|pair| {
        let (left, _) = pair[0];
        let (right, count) = pair[1];
        Rectangle::new([(left, 0u64), (right, count)], RGBColor(70, 130, 180).filled())
    })
)?;
root.present()?;
```

**关键约束：**
- `(min..max).log_scale()` — u64 实现 `LogScalable`，可直接使用 [VERIFIED: plotters source]
- `log_scale()` 返回 `LogRangeExt<u64>`，`build_cartesian_2d` 接受 `AsRangedCoord`
- `iter_recorded()` 只迭代非空 bucket（不含零计数），适合稀疏直方图 [VERIFIED: hdrhistogram docs]
- `v.count_at_value()` 返回 `T`（本项目为 `u64`）[VERIFIED: hdrhistogram docs]
- `v.value_iterated_to()` 返回 `u64` [VERIFIED: hdrhistogram docs]

### Pattern 4: TemplateAggregator 公开迭代接口

**What:** 为图表生成暴露内部 histogram 数据
**When to use:** `template_aggregator.rs` 新增方法

当前 `TemplateEntry` 为私有结构体，`histogram` 字段私有。`finalize()` 消耗 `self` 无法复用。需新增：

```rust
/// 供图表生成使用的模板条目引用（只读访问）
pub struct ChartEntry<'a> {
    pub key: &'a str,
    pub count: u64,
    pub histogram: &'a hdrhistogram::Histogram<u64>,
}

impl TemplateAggregator {
    /// 按 count 降序迭代所有模板条目（图表生成专用，在 finalize 前调用）
    pub fn iter_chart_entries(&self) -> impl Iterator<Item = ChartEntry<'_>> {
        // 需要排序后返回（与 finalize 的排序逻辑一致）
        let mut entries: Vec<_> = self.entries.iter()
            .map(|(key, entry)| ChartEntry {
                key: key.as_str(),
                count: entry.histogram.len(),
                histogram: &entry.histogram,
            })
            .collect();
        entries.sort_unstable_by(|a, b| b.count.cmp(&a.count));
        entries.into_iter()
    }
}
```

**注意：** 返回 `Vec::into_iter()` 而非直接返回引用迭代器，因排序需要分配。若 40 行限制有压力，排序逻辑可提取为辅助函数。

### Pattern 5: 文件名 sanitize

**What:** 将模板 key 转换为安全文件名
**When to use:** `latency_histogram_<key>.svg` 文件名生成

```rust
/// 将模板 key 转换为文件系统安全字符串（最长 80 字符）
fn sanitize_filename(key: &str) -> String {
    let sanitized: String = key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // 截断到 80 字符避免路径过长
    sanitized.chars().take(80).collect()
}
```

### Anti-Patterns to Avoid

- **返回 LogRange（deprecated）：** 使用 `.log_scale()` 方法代替 `LogRange::new()`（plotters 源码已标记 LogRange 为 deprecated）[VERIFIED: plotters source]
- **BitMapBackend：** 禁止引入任何 bitmap 相关 feature（会引入字体/图像系统依赖）
- **在 finalize() 之后访问 histogram：** `finalize()` 消耗 `self`，histogram 数据已丢失，必须在 finalize 前调用图表生成（D-01）
- **Histogram series 用于预计算 bucket：** plotters `Histogram` series 设计为对原始数据点自行聚合（data 接收 `(pos, 1)` 形式），不适合 hdrhistogram 输出的预计算 bucket；直方图应用手动 Rectangle
- **Y 轴标签区域太窄：** 模板 key 截断到 40 字符后仍需约 300px 宽度，否则标签溢出或截断

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SVG 图形生成 | 手写 SVG XML 字符串 | plotters SVGBackend | SVG 格式复杂，坐标变换、文本布局易出错 |
| histogram bucket 迭代 | 自行实现 bucket 提取 | `iter_recorded()` | HDR 的 bucket 边界计算涉及 sigfig 量化，手写必错 |
| 对数坐标变换 | 手动 `log10()` 坐标计算 | `(range).log_scale()` | 边界值（0、负数）处理复杂 |
| 文件目录创建 | 逐层检查并创建 | `std::fs::create_dir_all` | 已验证的标准库函数 |

**Key insight:** `ensure_parent_dir` 在 `src/exporter/mod.rs` 中定义为 `pub(super)`，仅对 `exporter` 子模块可见。`src/charts/` 模块应直接调用 `std::fs::create_dir_all(output_dir)` 或内联等价逻辑。

## Runtime State Inventory

> Phase 15 为新功能添加（新建模块 + 新增 SVG 输出），非 rename/refactor。此节略。

## Common Pitfalls

### Pitfall 1: TemplateEntry.histogram 私有导致编译错误
**What goes wrong:** `generate_charts(&agg, cfg)` 无法读取每个模板的 histogram 数据
**Why it happens:** `TemplateEntry` 为私有结构体，`histogram` 字段私有
**How to avoid:** 在 `template_aggregator.rs` 新增 `pub struct ChartEntry` 和 `pub fn iter_chart_entries()`，通过公共接口暴露只读访问
**Warning signs:** 编译器报 `field histogram of private type`

### Pitfall 2: log_scale() 与 0 值 bucket
**What goes wrong:** histogram 包含 value=0 的 bucket 时对数坐标崩溃（log(0) = -∞）
**Why it happens:** `hdrhistogram` 配置 `new_with_bounds(1, 60_000_000, 2)` 下限为 1，且 `iter_recorded()` 只迭代非空 bucket；但 `observe()` 中 `clamp(1, 60_000_000)` 确保输入 ≥ 1
**How to avoid:** 构建对数范围时用 `min_val.max(1)` 作为下界；`iter_recorded()` 本身已过滤零计数 bucket
**Warning signs:** `panicked at 'log of non-positive number'`

### Pitfall 3: plotters Histogram series 用于 hdrhistogram bucket 数据
**What goes wrong:** `Histogram::horizontal(&chart).data(buckets.iter().map(|b| (b.value, b.count)))` 产生错误结果，因 Histogram series 会再次聚合
**Why it happens:** plotters `Histogram` series 设计为对原始数据点做频率计数（如 `data(samples.map(|x| (x, 1)))`），而 hdrhistogram 输出的是已聚合 bucket
**How to avoid:** 耗时直方图使用手动 `Rectangle` 序列；频率条形图才用 `Histogram::horizontal`（其输入是 `(y_category_index, count)` 形式，count 直接作为 x 值）
**Warning signs:** 图表中所有柱高度为 1 或 bucket 数量异常

### Pitfall 4: SVGBackend 未调用 present()
**What goes wrong:** SVG 文件内容不完整或为空
**Why it happens:** SVGBackend 使用内部 buffer，`present()` 才将内容写入文件
**How to avoid:** 每个 draw 函数末尾显式调用 `root.present()?`（SC-4）
**Warning signs:** 生成的 SVG 文件可打开但图表元素缺失

### Pitfall 5: Y 轴标签 SegmentedCoord 索引映射
**What goes wrong:** `Histogram::horizontal` 使用 `(0..n).into_segmented()` 作为 Y 轴，`configure_mesh().y_label_formatter` 接收 `&SegmentValue<usize>`，无法直接格式化为模板 key
**Why it happens:** `into_segmented()` 将整数映射为区间端点，formatter 参数类型为 `SegmentValue`
**How to avoid:** 在 formatter closure 中 capture 模板 key 列表，根据 `SegmentValue::CenterOf(i)` 的 i 值查找 key；或使用 `SegmentValue::Exact(i)` 情况

```rust
// SegmentValue 类型需从 prelude 导入
use plotters::prelude::SegmentValue;
let labels: Vec<&str> = ...;  // top_n 个截断后的 key

chart.configure_mesh()
    .y_label_formatter(&|v: &SegmentValue<usize>| {
        match v {
            SegmentValue::CenterOf(i) => labels.get(*i).copied().unwrap_or("").to_string(),
            SegmentValue::Exact(i) => labels.get(*i).copied().unwrap_or("").to_string(),
            SegmentValue::Last => String::new(),
        }
    })
    .draw()?;
```

**Warning signs:** Y 轴显示数字索引而非模板 key 文字

### Pitfall 6: `ensure_parent_dir` 不可访问
**What goes wrong:** `use crate::exporter::ensure_parent_dir` 编译报错
**Why it happens:** `ensure_parent_dir` 定义为 `pub(super)`，仅对 `exporter` 子模块可见
**How to avoid:** `src/charts/mod.rs` 直接调用 `std::fs::create_dir_all(output_dir)?`

### Pitfall 7: 并行路径 generate_charts 调用时机
**What goes wrong:** 并行路径遗漏图表生成，或在 `parallel_agg` move 之后访问
**Why it happens:** `run.rs` 并行路径中 `parallel_agg.map(TemplateAggregator::finalize)` 会消耗聚合器
**How to avoid:** 按 CONTEXT.md 给出的调用顺序：先 `generate_charts(&agg, cfg)?`，再 `agg.finalize()`，与顺序路径对称

## Code Examples

### 完整频率条形图骨架（frequency_bar.rs）

```rust
// Source: D-07/D-08/D-09 + plotters 0.3.7 Histogram::horizontal API
use plotters::prelude::*;
use crate::features::ChartEntry;

const CHART_W: u32 = 1200;
const CHART_H: u32 = 600;
const STEELBLUE: RGBColor = RGBColor(70, 130, 180);

pub fn draw_frequency_bar(
    entries: &[ChartEntry<'_>],
    top_n: usize,
    output_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let data: Vec<(String, u64)> = entries.iter()
        .take(top_n)
        .map(|e| (truncate_label(e.key, 40), e.count))
        .collect();

    let max_count = data.iter().map(|(_, c)| *c).max().unwrap_or(1);
    let n = data.len();

    let root = SVGBackend::new(output_path, (CHART_W, CHART_H)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(format!("Top {} SQL Templates by Frequency", n), ("sans-serif", 20))
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(300)
        .build_cartesian_2d(0u64..max_count, (0..n).into_segmented())?;

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
            .style(STEELBLUE.filled())
            .margin(5)
            .data(data.iter().enumerate().map(|(i, (_, count))| (i, *count))),
    )?;

    root.present()?;
    Ok(())
}

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

### 完整耗时直方图骨架（latency_hist.rs）

```rust
// Source: hdrhistogram iter_recorded() + plotters Rectangle + (range).log_scale()
use plotters::prelude::*;
use hdrhistogram::Histogram;

pub fn draw_latency_hist(
    key: &str,
    histogram: &Histogram<u64>,
    output_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let buckets: Vec<(u64, u64)> = histogram
        .iter_recorded()
        .map(|v| (v.value_iterated_to(), v.count_at_value()))
        .collect();

    if buckets.is_empty() {
        return Ok(());  // 无数据，跳过
    }

    let min_val = buckets.first().map(|(v, _)| (*v).max(1)).unwrap_or(1);
    let max_val = buckets.last().map(|(v, _)| *v).unwrap_or(1);
    let max_count = buckets.iter().map(|(_, c)| *c).max().unwrap_or(1);

    let root = SVGBackend::new(output_path, (1200, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let truncated = truncate_key_display(key, 60);
    let mut chart = ChartBuilder::on(&root)
        .caption(format!("Latency: {}", truncated), ("sans-serif", 18))
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            (min_val..max_val).log_scale(),  // log X 轴
            0u64..max_count,
        )?;

    chart.configure_mesh()
        .x_desc("Latency (µs)")
        .y_desc("Count")
        .draw()?;

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

    root.present()?;
    Ok(())
}
```

### generate_charts 入口（charts/mod.rs）

```rust
// Source: D-12 + 项目已有 ensure_parent_dir 等价逻辑
use crate::features::{TemplateAggregator, ChartsConfig};
use crate::error::Result;

pub mod frequency_bar;
pub mod latency_hist;

pub fn generate_charts(agg: &TemplateAggregator, cfg: &ChartsConfig) -> Result<()> {
    let output_dir = std::path::Path::new(&cfg.output_dir);
    std::fs::create_dir_all(output_dir)
        .map_err(|e| /* 转换为 crate::error::Error */ ...)?;

    let entries: Vec<_> = agg.iter_chart_entries().collect();

    if cfg.frequency_bar {
        let path = output_dir.join("top_n_frequency.svg");
        frequency_bar::draw_frequency_bar(&entries, cfg.top_n, &path)?;
    }

    if cfg.latency_hist {
        for entry in entries.iter().take(cfg.top_n) {
            let filename = format!("latency_histogram_{}.svg",
                sanitize_filename(entry.key));
            let path = output_dir.join(&filename);
            latency_hist::draw_latency_hist(entry.key, entry.histogram, &path)?;
        }
    }
    Ok(())
}

fn sanitize_filename(key: &str) -> String {
    let sanitized: String = key.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    sanitized.chars().take(80).collect()
}
```

### ChartsConfig 结构（features/mod.rs 新增）

```rust
// Source: D-04/D-05 — 与 TemplateAnalysisConfig 对称模式
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ChartsConfig {
    pub output_dir: String,
    #[serde(default = "default_top_n")]
    pub top_n: usize,
    #[serde(default = "default_true")]
    pub frequency_bar: bool,
    #[serde(default = "default_true")]
    pub latency_hist: bool,
}

fn default_top_n() -> usize { 10 }
// default_true() 已在 mod.rs 定义，可复用
```

### Config::validate() 新增检查（config.rs）

```rust
// Source: D-06 — charts 启用时 template_analysis.enabled 必须为 true
if self.features.charts.is_some() {
    let ta_enabled = self.features.template_analysis
        .as_ref()
        .is_some_and(|ta| ta.enabled);
    if !ta_enabled {
        return Err(Error::Config(ConfigError::InvalidValue {
            field: "features.charts".to_string(),
            value: String::new(),
            reason: "启用 [features.charts] 需要先设置 [features.template_analysis]\nenabled = true".to_string(),
        }));
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `LogRange::new(min, max)` | `(min..max).log_scale()` | plotters 0.3+ | LogRange 被标记为 deprecated，应使用 IntoLogRange trait |
| BitMapBackend (全功能) | SVGBackend + `default-features = false` | — | 无字体/图像系统依赖，适合 CLI 工具 |

**Deprecated/outdated:**
- `plotters::coord::LogRange`：已标记 deprecated，使用 `IntoLogRange::log_scale()` [VERIFIED: plotters source]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | plotters 0.3.7 是 crates.io 当前最新稳定版 | Standard Stack | 需在写 Cargo.toml 时再次 `cargo search plotters` 确认 |
| A2 | plotters SVG-only 最小 features 为 `["svg_backend", "all_series", "all_elements"]` | Standard Stack | 编译报 missing feature；实际可能只需 `["svg_backend", "all_series"]`（Rectangle 在 all_elements 中）；planner 应在 Wave 0 验证编译 |
| A3 | `SegmentValue::CenterOf` 是 `into_segmented()` Y 轴 formatter 的主要 variant | Code Examples | 若实际为其他 variant，Y 轴标签显示为空；可通过 dbg! 快速确认 |
| A4 | plotters 0.3.7 (marked [ASSUMED] 因 slopcheck 不可用) 无 supply chain 风险 | Package Legitimacy | 极低（159M 下载，Criterion 依赖），但形式上未经 slopcheck 验证 |

**If this table is empty:** N/A — 有 4 条 assumed 项。

## Open Questions

1. **`SegmentValue` variants 精确匹配**
   - What we know: `into_segmented()` Y 轴的 label formatter 接收 `SegmentValue<usize>` 参数
   - What's unclear: 实际触发的 variant 是 `CenterOf` 还是 `Exact`，取决于 plotters 内部实现
   - Recommendation: Wave 0 任务中用 `eprintln!("{:?}", v)` 打印实际 variant，调整 formatter

2. **图表中标签 RTL（右到左）渲染**
   - What we know: SVGBackend 使用系统字体，中文模板 key 可能渲染为方框
   - What's unclear: plotters SVG-only 模式（无 ttf feature）是否支持中文文字
   - Recommendation: Y 轴标签已截断到 40 字符，若含中文则 sanitize_filename 会替换为 `_`；频率图的 Y 轴标签截断函数用 `.chars()` 迭代（Unicode-safe），字体问题留 Wave 0 观察

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust / cargo | 编译 | ✓ | （项目已有） | — |
| plotters 0.3.7 | SVG 图表生成 | 需下载 | 0.3.7 | — |
| std::fs::create_dir_all | 目录创建 | ✓ | std | — |

**Missing dependencies with no fallback:** plotters（需在 Cargo.toml 新增）
**Missing dependencies with fallback:** 无

## Validation Architecture

> `workflow.nyquist_validation` 未明确设置 false，按启用处理。

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust 内置 `#[test]` + `tempfile` crate（已在 dev-dependencies） |
| Config file | 无独立配置文件（Cargo.toml `[[test]]` 发现） |
| Quick run command | `cargo test charts` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CHART-01 | charts.output_dir 目录被创建，SVG 文件写入 | integration | `cargo test charts::tests::test_generate_charts_creates_output_dir` | ❌ Wave 0 |
| CHART-01 | charts 未启用时不创建 output_dir（SC-5） | integration | `cargo test charts::tests::test_no_dir_when_disabled` | ❌ Wave 0 |
| CHART-02 | top_n_frequency.svg 存在且非空（浏览器可打开） | integration | `cargo test charts::tests::test_frequency_bar_file_not_empty` | ❌ Wave 0 |
| CHART-03 | latency_histogram_*.svg 文件数量 = top_n（各模板一张） | integration | `cargo test charts::tests::test_latency_hist_count` | ❌ Wave 0 |
| D-06 | charts + template_analysis.enabled=false → 验证报错 | unit | `cargo test config::tests::test_validate_charts_requires_template_analysis` | ❌ Wave 0 |
| D-08 | truncate_label 超过 40 字符时截断 + `…` | unit | `cargo test charts::tests::test_truncate_label` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test`
- **Per wave merge:** `cargo test && cargo clippy --all-targets -- -D warnings`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/charts/mod.rs` — 模块骨架 + `generate_charts` + `sanitize_filename` 单元测试
- [ ] `src/charts/frequency_bar.rs` — `draw_frequency_bar` + `truncate_label` 单元测试
- [ ] `src/charts/latency_hist.rs` — `draw_latency_hist` 骨架
- [ ] `src/features/template_aggregator.rs` — `ChartEntry` + `iter_chart_entries()` 单元测试
- [ ] `src/features/mod.rs` — `ChartsConfig` 结构体 + serde 单元测试
- [ ] `src/config.rs` — D-06 验证逻辑单元测试

## Security Domain

> `security_enforcement` 未显式设置，按启用处理。

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | sanitize_filename（文件名 sanitize）；D-06 validate() 跨字段检查 |
| V6 Cryptography | no | — |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| 文件路径穿越（../../../etc）| Tampering | `output_dir` 为用户配置字段；写入前不做相对路径防御；本项目 CLI 工具，用户可信。sanitize_filename 对 latency_hist 文件名做字符白名单，防止 key 中含 `../` |
| 模板 key 过长导致文件系统路径超限 | DoS | sanitize_filename 截断到 80 字符 |

## Sources

### Primary (HIGH confidence)
- [plotters 0.3.7 docs.rs](https://docs.rs/plotters/0.3.7/plotters/) — SVGBackend API, ChartBuilder, Histogram series
- [plotters source: logarithmic.rs](https://github.com/plotters-rs/plotters/blob/a212c30a17f0c44f683b44adb096bba3bae21ae5/plotters/src/coord/ranged1d/combinators/logarithmic.rs) — LogScalable impl for u64, IntoLogRange, log_scale() method, LogRange deprecated
- [hdrhistogram 7.5.4 docs.rs](https://docs.rs/hdrhistogram/7.5.4/hdrhistogram/struct.Histogram.html) — iter_recorded() signature
- [hdrhistogram IterationValue](https://docs.rs/hdrhistogram/7.5.4/hdrhistogram/iterators/struct.IterationValue.html) — value_iterated_to(), count_at_value() methods
- crates.io API — plotters 0.3.7 (159M downloads), hdrhistogram 7.5.4 (86M downloads) [VERIFIED]

### Secondary (MEDIUM confidence)
- [plotters Cargo.toml features](https://docs.rs/crate/plotters/latest/source/Cargo.toml.orig) — feature flags: svg_backend, all_series, all_elements, default set
- [plotters histogram.rs example](https://raw.githubusercontent.com/plotters-rs/plotters/master/plotters/examples/histogram.rs) — Histogram::vertical 完整示例（horizontal 用法对称）
- [plotters IntoLogRange 使用示例](https://www.k-pmpstudy.com/entry/2022/11/20/rustPlottersLog) — `IntoLogRange::log_scale(range)` 语法验证

### Tertiary (LOW confidence)
- WebSearch: hdrhistogram iter_recorded IterationValue methods — 与官方文档交叉验证

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — crates.io 直接验证版本号
- Architecture: HIGH — 基于已有代码深度分析
- plotters API: MEDIUM-HIGH — docs.rs + 源码双重验证，SegmentValue variant 部分为 ASSUMED
- hdrhistogram API: HIGH — docs.rs 直接验证方法签名
- Pitfalls: HIGH — 基于 API 分析推断，部分经 plotters 源码确认

**Research date:** 2026-05-16
**Valid until:** 2026-06-16（plotters 0.3.x API 稳定，hdrhistogram 7.5.x 已锁定）

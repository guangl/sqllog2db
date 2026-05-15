# Pitfalls: v1.3 SQL 模板分析 & SVG 图表

**Domain:** Rust streaming CLI — adding SQL template normalization, statistical aggregation, and SVG chart generation
**Researched:** 2026-05-15
**Confidence:** HIGH (derived from direct codebase inspection of all hot paths + Rust ecosystem patterns)

---

## Severity Classification

| Severity | Meaning |
|----------|---------|
| CRITICAL | Causes data corruption, unbounded memory growth, or silent regression on existing tests |
| HIGH | Performance regression >5% on hot path (D-G1 gate), or incorrect normalization keys producing wrong stats |
| MEDIUM | Footgun during implementation that requires rework, but testable and recoverable |
| LOW | Maintenance/correctness risk visible only under specific conditions |

---

## Pitfall Table

| # | Name | Severity | Category | Phase |
|---|------|----------|----------|-------|
| 1 | 统计累积器加入管线破坏 `pipeline.is_empty()` | CRITICAL | Integration | TMPL-02 |
| 2 | `Vec<u64>` 每模板无界增长造成 OOM | CRITICAL | Memory | TMPL-02 |
| 3 | 归一化在现有 `normalized_sql` 路径之外另算，产生不一致 key | HIGH | Correctness | TMPL-01 |
| 4 | IN 列表归一化产生错误 key（括号/注释边界） | HIGH | Correctness | TMPL-01 |
| 5 | 百分位数计算在流式路径中无法直接算（需要排序） | HIGH | Correctness | TMPL-02 |
| 6 | SVG 生成字符串拼接造成热路径外的不可控内存峰值 | HIGH | Memory | CHART-01 |
| 7 | 统计输出写入与现有 exporter 生命周期不同步 | HIGH | Integration | TMPL-03/04 |
| 8 | 新 config 字段破坏现有 TOML 向后兼容 | MEDIUM | Config | TMPL-01 |
| 9 | 模板 key 大小写/空白不一致导致聚合分裂 | MEDIUM | Correctness | TMPL-01 |
| 10 | `HashMap<String, TemplateStats>` key 分配在热循环 | MEDIUM | Performance | TMPL-02 |
| 11 | SVG 文件句柄未在 finalize 前关闭，内容截断 | MEDIUM | Resource | CHART-01 |
| 12 | 统计处理器调用 `process()` 但需要改变状态（trait 错用） | MEDIUM | Design | TMPL-02 |
| 13 | 并行 CSV 路径下统计累积器跨线程竞争 | HIGH | Concurrency | TMPL-02 |
| 14 | `apply_overrides` 未覆盖新 config key，导致 `--set` 不生效 | LOW | Config | TMPL-01 |
| 15 | 模板统计文件路径未在 `validate()` 阶段检查，运行中才报错 | LOW | Config | TMPL-03 |

---

## Critical Pitfalls

### Pitfall 1: 统计累积器加入管线破坏 `pipeline.is_empty()` 零开销快路径

**Severity:** CRITICAL

**What goes wrong:**
`TemplateStatsProcessor` 无论用户是否配置 `[features.template_stats]`，只要代码中无条件调用 `pipeline.add(Box::new(TemplateStatsProcessor::new()))` 就会让 `pipeline.is_empty()` 永远返回 false。

结果：`process_log_file` 热循环中每条记录都经过管线，触发 `record.parse_meta()` + 虚函数调用，**无过滤配置基准从 ~5.2M records/sec 退化**，D-G1 门控（>5%）将被触发。

**Why it happens:**
统计累积器与过滤器不同——过滤器是"无规则时不加"，而统计器是"用户想要统计时才加"。但开发者容易写成：

```rust
// 错误：TemplateStatsProcessor 被无条件添加
pipeline.add(Box::new(TemplateStatsProcessor::new()));
```

**Prevention:**
完全复制现有 `build_pipeline()` 中的守卫模式。只有当 `cfg.features.template_stats` 存在且 `enable == true` 时才添加：

```rust
if let Some(ts_cfg) = &cfg.features.template_stats {
    if ts_cfg.enable {
        pipeline.add(Box::new(TemplateStatsProcessor::new(ts_cfg)));
    }
}
```

无过滤 + 无统计的现有配置走 `pipeline.is_empty() == true` 快路径，完全不受影响。

**Warning signs:**
- `cargo criterion --bench bench_csv` 在未配置 template_stats 时性能退化 >5%
- 单元测试 `assert!(pipeline.is_empty())` 在 `FeaturesConfig::default()` 下失败

**Phase:** TMPL-02 实现前先写这个守卫的单元测试

---

### Pitfall 2: `Vec<u64>` 每模板无界增长造成 OOM

**Severity:** CRITICAL

**What goes wrong:**
若为每个模板存储全量耗时样本用于精确百分位计算：

```rust
struct TemplateStats {
    exec_times: Vec<u64>,  // 无界！
    ...
}
```

**具体内存计算（针对本项目规模）：**

- 真实基准：1.1GB 文件，~1.55M records/sec，假设 1 小时日志 ≈ 5M 条记录
- 假设 10 个高频模板各命中 500K 条：10 × 500K × 8 bytes = **40 MB**
- 假设 1000 个模板各命中 5K 条：1000 × 5K × 8 bytes = **40 MB**
- 极端场景：1 个热模板命中全部 5M 条：5M × 8 bytes = **40 MB**（单模板）
- 真实危险：100 个模板各 50K 条 = 40MB，但叠加 `HashMap<String, TemplateStats>` key 堆分配后，总内存开销可达 **200~400 MB**，在 4GB 以下机器上触发 OOM 或 swap 抖动

设计承诺是"流式处理，内存恒定"——`Vec<u64>` 直接打破这个承诺。

**Prevention:**
使用近似百分位算法，不存储全量样本：

**选项 A（推荐）：固定尺寸直方图（t-digest lite）**
按耗时范围分桶，如 64 个桶，每个模板仅占 64×8 = 512 bytes。p50/p95/p99 误差 <5%，内存 O(桶数)，与模板数量无关（模板数量本身有界）。

**选项 B：DDSketch（近似百分位，误差界 ε=0.01）**
Rust 生态没有成熟的 no-std DDSketch，需要手动实现或引入 `quantiles` crate（依赖较重）。

**选项 C（不推荐）：流式中位数（堆方法）**
只能算 p50，不支持 p95/p99。

**推荐实现：**
```rust
struct TemplateStats {
    count: u64,
    sum_ms: u64,
    min_ms: u64,
    max_ms: u64,
    // 固定 64 桶直方图：桶 i 计 [2^i, 2^(i+1)) ms 的次数
    histogram: [u32; 64],
}
```

64 桶覆盖 1ms 到 ~580 年，足以表示所有实际 SQL 耗时。每模板 64×4 + 4×8 = **288 bytes**，10K 模板 = **2.8 MB**，内存完全可控。

**Warning signs:**
- `struct TemplateStats` 中出现 `Vec<u64>` 或 `Vec<f32>` 字段
- 运行 100 万记录后 `/usr/bin/time -v` 显示 RSS 超过 500 MB

**Phase:** TMPL-02 设计阶段必须做出选择，不能留到实现后再改

---

## High Severity Pitfalls

### Pitfall 3: 归一化在现有 `normalized_sql` 路径之外另算，产生不一致 key

**Severity:** HIGH

**What goes wrong:**
现有代码路径：`compute_normalized()` → `apply_params_into()` 产生带参数值的 `normalized_sql`（参数已替换为实际值）。

模板 key 需要的是参数位置统一为占位符（如 `?`）的 fingerprint，这正是 `src/features/sql_fingerprint.rs` 中 `fingerprint()` 函数做的事。

如果 TMPL-01 在 `TemplateStatsProcessor::process()` 中自己再写一套归一化逻辑，就产生两套并行存在的归一化代码，容易出现：
- 同一条 SQL 被两套逻辑产生不同的 key
- 注释处理方式不一致（一个去注释，一个不去）
- 空白折叠规则不一致

**Prevention:**
直接复用 `features::fingerprint(sql)` 作为模板 key 生成函数。`fingerprint()` 已经做了：字符串字面量替换为 `?`、数字替换为 `?`、连续空白折叠。这是标准的 SQL fingerprint，不需要重复发明。

如果 TMPL-01 需要额外的注释去除或大小写统一，应在 `sql_fingerprint.rs` 中扩展 `fingerprint()` 或添加 `normalize_for_template()` 函数，而不是在 `TemplateStatsProcessor` 中内联实现。

**Warning signs:**
- `TemplateStatsProcessor` 中出现自己实现的 `to_lowercase()` + `trim()` + 正则替换逻辑
- 两条明显相同结构的 SQL 在统计输出中出现为两个不同模板

**Phase:** TMPL-01 实现时，必须先确认 `fingerprint()` 的覆盖范围是否满足需求，缺什么在 `sql_fingerprint.rs` 中统一扩展

---

### Pitfall 4: IN 列表归一化产生错误 key（括号嵌套/多值处理）

**Severity:** HIGH

**What goes wrong:**
SQL 模板归一化的常见需求之一是将 `IN (1, 2, 3)` 和 `IN (1, 2, 3, 4, 5)` 归一化为同一模板。这看似简单，但有多个边界情况：

1. **嵌套括号：** `IN (SELECT id FROM t WHERE x = 1)` 不应被归一化
2. **函数调用：** `COALESCE(a, b, c)` 不应被归一化
3. **字符串中的括号：** `WHERE note = 'IN (1,2)'` 的括号在字符串字面量内部
4. **已被 fingerprint 处理的数字：** fingerprint 已将 `1, 2, 3` 变成 `?, ?, ?`，IN 列表归一化若在 fingerprint 之前运行，会引入二次处理

**Prevention:**
- IN 列表归一化必须在字符串字面量替换之后运行，且只处理被 `?,` 分隔的参数列表
- 最安全的做法：fingerprint 之后，用正则 `IN\s*\(\s*\?(?:\s*,\s*\?)*\s*\)` 匹配并替换为 `IN (?+)`
- 或者完全跳过 IN 列表归一化——现有 `fingerprint()` 已将数字替换为 `?`，`IN (1,2,3)` 和 `IN (4,5,6)` 已经得到相同的 fingerprint `IN (?, ?, ?)`，只是 `IN (1,2)` 和 `IN (1,2,3)` 得到不同指纹（参数数量不同）。这对大多数场景是可接受的。

**Warning signs:**
- 测试用例：`IN ('a', 'b')` 和 `IN ('c')` 被错误地归为同一模板（括号内容不同但数量也不同）
- 子查询 `IN (SELECT ...)` 被匹配导致整个子查询被删除

**Phase:** TMPL-01 设计阶段就需要决定是否实现 IN 归一化，以及采用哪种实现策略；可以作为可选步骤后置

---

### Pitfall 5: 百分位数计算在流式单遍路径中无法直接算（需要全量数据排序）

**Severity:** HIGH

**What goes wrong:**
精确的 p95/p99 计算需要对全量样本排序后取第 95/99 百分位，这本质上是 O(n log n) 且需要 O(n) 内存存储全量数据。在流式单遍架构中，每条记录处理完就丢弃，无法回头排序。

如果实现者没有意识到这个约束，会在处理完所有记录后调用 `Vec::sort()` 然后取索引，强迫系统把全量数据收集到 `Vec<u64>` 中——这直接触发 Pitfall 2（OOM）。

**Prevention:**
在实现设计文档中明确声明：p50/p95/p99 使用近似算法（直方图插值），精度说明写入文档（"基于 64 桶指数直方图，误差 <5%"）。

直方图插值方式：
```
// 桶 i 覆盖 [2^i, 2^(i+1)) ms
// 若 p95 对应第 k 个样本落在桶 i，线性插值得近似值
fn percentile_from_histogram(hist: &[u32; 64], total: u64, p: f64) -> u64 {
    let target = (total as f64 * p).ceil() as u64;
    let mut cumulative = 0u64;
    for (i, &count) in hist.iter().enumerate() {
        cumulative += count as u64;
        if cumulative >= target {
            return 1u64 << i; // 桶中点近似
        }
    }
    u64::MAX
}
```

**Warning signs:**
- 实现中出现 "排序后取百分位" 注释
- `TemplateStats` 有 `exec_times: Vec<u64>` 字段（见 Pitfall 2）

**Phase:** TMPL-02 设计阶段，必须在第一个实现 step 中写清楚近似方案

---

### Pitfall 6: SVG 生成字符串拼接造成内存峰值和性能问题

**Severity:** HIGH

**What goes wrong:**
SVG 是纯文本格式。朴素实现会为每个 SVG 元素创建一个 `String`，然后通过 `+` 或 `format!` 拼接，最后一次性写入文件。在生成 Top N 条形图（N=50 时 SVG 约 50KB）时，可能存在多次完整副本在内存中同时存在（`format!` 创建临时 String → 拼接到大 String → 写入文件 → 临时 String 释放）。

更大的问题：**如果 SVG 生成逻辑混入热循环（即 `process()` 中生成 SVG），会完全破坏流式处理性能。** SVG 必须在全部记录处理完毕（finalize 阶段）后才能生成，因为图表数据来自聚合统计。

**Prevention:**
- SVG 生成**只在 finalize 阶段**运行，绝不在热循环的 `process()` 中
- 使用 `BufWriter<File>` 直接写入，避免内存中持有完整 SVG 字符串
- 每个 SVG 元素（`<rect>`, `<text>` 等）通过 `write!` 直接输出到 `BufWriter`，不中间存储
- 如果使用 SVG 生成库（如 `svg` crate），确认其 API 是否支持流式写出；否则用手工 `write!` 宏

**推荐代码模式：**
```rust
// 在 finalize() 中，而不是在 process() 中
fn write_bar_chart(&self, path: &Path, stats: &[TemplateStats]) -> Result<()> {
    let file = File::create(path)?;
    let mut w = BufWriter::with_capacity(64 * 1024, file);
    write!(w, r#"<svg xmlns="...">"#)?;
    for (i, s) in stats.iter().take(self.top_n).enumerate() {
        write!(w, r#"<rect x="{}" y="{}" .../>"#, i * 20, s.count)?;
    }
    write!(w, "</svg>")?;
    w.flush()?;  // 必须显式 flush，否则 BufWriter drop 时的错误被静默丢弃
    Ok(())
}
```

**Warning signs:**
- `TemplateStatsProcessor::process()` 中出现任何 SVG 相关代码
- 在 finalize 外部出现 `svg::Document::new()` 或 `format!("<svg...")` 调用
- 生成 SVG 后没有显式 `flush()` 调用

**Phase:** CHART-01 实现阶段；在 design step 中明确 finalize-only 约束

---

### Pitfall 7: 统计输出写入与现有 exporter 生命周期不同步

**Severity:** HIGH

**What goes wrong:**
现有 exporter 生命周期为：`ExporterManager::initialize()` → 热循环 `export_one_preparsed()` → `ExporterManager::finalize()`。

TMPL-03（独立 JSON/CSV 报告）和 TMPL-04（SQLite `sql_templates` 表 / CSV 伴随文件）需要在热循环结束后写入数据。可能出现：

1. **统计数据在 finalize 之前被写出**：数据不完整（只有部分记录的统计）
2. **SQLite 统计表在主事务提交之前写入**：若主事务回滚，统计表仍然存在，造成数据不一致
3. **CSV 伴随文件路径冲突**：`_templates.csv` 写入时主 CSV 文件 `BufWriter` 仍然持有文件锁（Windows 上可能冲突）
4. **并行路径下统计数据分散**：`process_csv_parallel` 中每个线程有独立的 `ExporterManager`，统计数据无法在 finalize 时合并（各自 finalize 各自的独立分片）

**Prevention:**
- 统计输出必须作为 `finalize()` 的一部分，在主 exporter finalize 之后运行
- 对于 TMPL-04 SQLite 统计表：在同一个 `rusqlite::Connection` 的同一事务中写入，或在主事务提交后开新事务写统计表
- 对于并行路径：`TemplateStatsProcessor` 需要实现跨线程合并接口（`merge(&mut self, other: Self)`），主线程在 `process_csv_parallel` 返回后合并所有线程的统计数据，然后再写出
- CSV 伴随文件路径：主 CSV 写完（flush + close）之后才开写伴随文件

**Warning signs:**
- `process_csv_parallel` 返回后没有统计合并步骤
- `SqliteExporter::finalize()` 在提交主事务之前调用 `write_template_stats()`
- 伴随文件写入前主 CSV 的 `BufWriter` 还没 drop

**Phase:** TMPL-03/04 实现阶段；并行路径的合并接口需要在 TMPL-02 设计时就规划

---

### Pitfall 13: 并行 CSV 路径下统计累积器跨线程竞争

**Severity:** HIGH

**What goes wrong:**
`process_csv_parallel` 为每个文件创建独立线程，各持自己的 `ExporterManager`。若 `TemplateStatsProcessor` 是全局共享的（如通过 `Arc<Mutex<TemplateStatsAggregator>>`），所有线程在每条记录时争抢同一把锁，完全消除并行收益，甚至因锁竞争比单线程更慢。

**Prevention:**
每个线程持有独立的 `TemplateStatsProcessor` 实例（不共享），在 `process_csv_parallel` 结束后，由主线程依次合并各线程的统计结果：

```rust
// 每个并行 task 返回自己的 TemplateStats
type TaskResult = Option<(PathBuf, PathBuf, usize, TemplateStatsSnapshot)>;

// 主线程合并
let mut global_stats = TemplateStatsAggregator::new();
for stats_snapshot in thread_stats {
    global_stats.merge(stats_snapshot);
}
global_stats.write_output(&cfg)?;
```

这要求 `TemplateStatsAggregator` 实现 `merge()` 方法（HashMap 合并），是标准的 map-reduce 模式。

**Warning signs:**
- `Arc<Mutex<...>>` 出现在 `TemplateStatsProcessor` 或相关聚合器中
- 并行处理时 CPU 利用率比不用统计时低（锁竞争）

**Phase:** TMPL-02 设计阶段，在决定数据结构时就确定线程模型

---

## Medium Severity Pitfalls

### Pitfall 8: 新 config 字段破坏现有 TOML 向后兼容

**Severity:** MEDIUM

**What goes wrong:**
如果新增 `[features.template_stats]` 或 `[chart]` 配置段时忘记给结构体加 `#[serde(default)]`，则现有不包含这些字段的 TOML 文件在反序列化时会报错，所有 729 个集成测试中使用 `toml::from_str` 构建 config 的都会失败。

**Prevention:**
所有新的可选 config 字段必须：
1. 在 `FeaturesConfig` / `Config` 中用 `Option<T>` 类型（缺失 = None = 功能关闭）
2. 或在结构体级别加 `#[serde(default)]`（缺失 = `Default::default()`）

参考现有模式：`pub filters: Option<FiltersFeature>` — 缺失时为 `None`，整个功能关闭，不影响现有配置。

**不要**用非 Option 类型 + 无默认值来表示新功能：
```rust
// 错误：现有 TOML 文件反序列化失败
pub template_stats: TemplateStatsConfig,

// 正确：可选，缺失时功能关闭
pub template_stats: Option<TemplateStatsConfig>,
```

**Warning signs:**
- `cargo test` 在修改 `FeaturesConfig` 后出现 `missing field` serde 错误
- `toml::from_str("")` 对 `FeaturesConfig` 失败

**Phase:** 所有 config 结构体变更阶段（TMPL-01 first step）

---

### Pitfall 9: 模板 key 大小写/空白不一致导致聚合分裂

**Severity:** MEDIUM

**What goes wrong:**
DaMeng（达梦）SQL 日志中同一模板可能以不同大小写出现（`SELECT` vs `select`，`WHERE` vs `where`）。如果 key 生成时不统一大小写，同一逻辑模板会被统计为多个不同模板，导致 Top N 结果失真。

现有 `fingerprint()` 函数**不做大小写归一化**，因为它设计用于 `digest` 命令（保留原始大小写用于展示）。但模板统计需要归一化大小写来聚合。

**Prevention:**
模板 key 生成应在 `fingerprint()` 的输出上再做 `to_lowercase()`：

```rust
let template_key = fingerprint(sql_text).to_lowercase();
```

注意：`to_lowercase()` 在 Rust 中是 O(n) 的 heap 分配操作。如果模板数量有限（通常 <10K），在 key 插入 HashMap 时执行一次是可接受的。不要在热循环中每条记录都 `to_lowercase()` 整个 SQL，只在首次见到该 key 时做。

**Warning signs:**
- 统计报告中 `SELECT * FROM users` 和 `select * from users` 作为两个独立模板出现
- `TemplateStatsAggregator` 的 HashMap key 是原始 SQL 未经大小写处理

**Phase:** TMPL-01 实现阶段，key 生成函数需要测试用例覆盖大小写差异

---

### Pitfall 10: `HashMap<String, TemplateStats>` key 分配在热循环

**Severity:** MEDIUM

**What goes wrong:**
每条记录调用 `aggregator.get_or_insert(key)` 时，若使用 `HashMap<String, TemplateStats>`，会对每条记录调用 `fingerprint(sql)` 生成新的 `String`（heap 分配），然后调用 `HashMap::entry(key)` 查找。对于热模板（1M+ 次命中），这意味着 1M+ 次 `String::new` + hash + 比较 + drop。

现有代码使用 `compact_str::CompactString` 和 `ahash::HashMap` 来优化类似场景（见 `replace_parameters.rs`）。

**Prevention:**
- HashMap 使用 `ahash::HashMap`（已是项目依赖，hash 比 `std::HashMap` 快 ~2x）
- 对热路径查找使用 `entry()` API 避免二次查找
- 考虑使用 `compact_str::CompactString` 作为 key（<=23 字节的 fingerprint 可内联存储，但大多数 SQL fingerprint 超过 23 字节，实际收益有限）
- 更重要的优化：先检查 `pipeline.is_empty()` 守卫（见 Pitfall 1），没有统计需求时根本不进管线

**Warning signs:**
- `TemplateStatsAggregator` 使用 `std::collections::HashMap`（而非 `ahash::HashMap`）
- 每条记录都重新计算完整 fingerprint 即使该 key 已存在

**Phase:** TMPL-02 实现阶段；如果 D-G1 门控被触发再优化

---

### Pitfall 11: SVG 文件句柄未显式 flush，内容截断

**Severity:** MEDIUM

**What goes wrong:**
Rust 的 `BufWriter` 在 `drop` 时不会自动 flush（与 `Write::flush()` 不同）。如果 SVG 生成函数返回 `Ok(())` 但未调用 `flush()`，`BufWriter` 的缓冲区内容会在 drop 时静默丢失，导致生成的 SVG 文件末尾缺少 `</svg>` 标签，浏览器无法渲染。

这个 Rust 特性与大多数语言不同，容易被忽视。

**Prevention:**
所有 `BufWriter<File>` 写完后必须显式调用 `flush()`：

```rust
writer.flush().map_err(|e| Error::Io(e))?;
// 或者
use std::io::Write as _;
writer.flush()?;
```

在 SVG 写出函数的末尾加 `#[must_use]` 或通过返回 `Result` 强制调用方检查错误。

**Warning signs:**
- SVG 写出函数的末尾没有 `writer.flush()` 调用
- 生成的 SVG 文件偶尔在小数据集下完整、大数据集下末尾截断

**Phase:** CHART-01 实现阶段；code review checklist 加入 "BufWriter flush" 检查

---

### Pitfall 12: `LogProcessor::process()` trait 签名不允许累积状态（设计冲突）

**Severity:** MEDIUM

**What goes wrong:**
现有 `LogProcessor` trait 签名为：

```rust
pub trait LogProcessor: Send + Sync + std::fmt::Debug {
    fn process(&self, record: &Sqllog) -> bool;
    fn process_with_meta(&self, record: &Sqllog, meta: &MetaParts<'_>) -> bool;
}
```

注意：接收器是 `&self`（共享引用，不可变）。统计累积器需要在每次 `process()` 时**修改内部状态**（计数、累加耗时）。

用 `&self` 是不可能直接修改字段的。可能的解决方案：

1. **`&self` + `RefCell<Stats>` 内部可变性**：可行，但 `RefCell` 不是 `Sync`，而 trait bound 要求 `Sync`（因为 pipeline 在并行路径中跨线程传递）
2. **`&self` + `Mutex<Stats>`**：满足 `Sync`，但每次 `process()` 都要 lock/unlock，热路径性能下降
3. **改 trait 签名为 `&mut self`**：与现有所有实现不兼容（需要改 `Pipeline::run_with_meta` 为 `&mut self` + 每次借用 `&mut` processor），破坏所有 729 个使用 `pipeline` 的测试
4. **统计处理器不实现 `LogProcessor`，而是单独的 `LogSink` trait**：推荐方案，见下方

**Prevention:**
统计累积器不要实现 `LogProcessor` trait（该 trait 设计用于记录过滤，返回 `bool`）。改为设计独立的 `StatsSink` 或 `RecordVisitor` trait：

```rust
pub trait RecordVisitor {
    fn visit(&mut self, record: &Sqllog, meta: &MetaParts<'_>, pm: &PerformanceMetrics<'_>);
}
```

在 `process_log_file` 热循环中，统计访问器在 exporter 写出之后调用：

```rust
exporter_manager.export_one_preparsed(&record, &meta, &pm, ns)?;
if let Some(visitor) = stats_visitor.as_mut() {
    visitor.visit(&record, &meta, &pm);
}
```

这保留了 `pipeline.is_empty()` 快路径的完整性，且统计访问器使用 `&mut self` 可以自由修改状态。

**Warning signs:**
- `TemplateStatsProcessor` 实现了 `LogProcessor` trait
- 代码中出现 `Mutex<TemplateStats>` 在 `LogProcessor::process` 内部
- `Pipeline` struct 的内部结构被修改（加 `&mut`）

**Phase:** TMPL-02 设计阶段（第一步，在动代码之前确定接口）

---

## Low Severity Pitfalls

### Pitfall 14: `apply_overrides()` 未覆盖新 config key

**Severity:** LOW

**What goes wrong:**
`Config::apply_one()` 中的 `match key { ... _ => return Err(unknown()) }` 要求所有合法的 `--set key=value` key 都在此处枚举。若新增 `features.template_stats.enable` 或 `chart.output_dir` 而未在 `apply_one()` 中添加对应分支，CLI 的 `--set` 功能对新字段无效，用户会得到 "unknown config key" 错误，但实际上 key 是合法的。

**Prevention:**
每次在 `FeaturesConfig` 或 `Config` 中添加新的顶级字段，同步在 `apply_one()` 中添加对应分支。添加一个集成测试覆盖新 key 的 `--set` 路径。

**Phase:** TMPL-01 和 CHART-01 config 阶段，在添加 config struct 的同一 commit 中更新 `apply_one`

---

### Pitfall 15: 统计文件路径未在 `validate()` 阶段检查

**Severity:** LOW

**What goes wrong:**
若 `template_stats.output` 路径指向一个不存在的目录（如 `/data/reports/stats.json` 但 `/data/reports/` 不存在），错误只在运行结束、调用 `finalize()` 时才出现。此时已经处理了全部记录（耗时可能数分钟），只是在写出统计结果时报错，造成用户体验极差（"跑了十分钟发现输出目录不存在"）。

**Prevention:**
在 `Config::validate()` / `validate_and_compile()` 中检查统计输出路径的父目录是否可写，或者至少检查路径非空。参考现有 `CsvExporter::validate()` 的模式（只检查路径非空，不要求文件存在，但确保在 initialize 阶段 `ensure_parent_dir()` 被调用）。

**Phase:** TMPL-03 config 阶段；在 `TemplateStatsConfig::validate()` 中实现

---

## Integration Gotchas

| Integration Point | Common Mistake | Correct Approach |
|------------------|----------------|------------------|
| `pipeline.is_empty()` | 统计处理器无条件加入管线 | 只在 `cfg.features.template_stats.enable == true` 时才添加；或者不用 `LogProcessor` trait，改用单独的 visitor 路径（见 Pitfall 12） |
| `process_csv_parallel` | 统计数据在各线程中分散，finalize 时只有最后一个线程的数据 | 每线程独立统计，主线程 merge（map-reduce 模式） |
| `ExporterManager::finalize()` | 统计表 / 伴随文件在主 exporter finalize 之前写出，数据不完整 | 统计写出在主 exporter finalize 之后，作为独立步骤 |
| `fingerprint()` 函数 | 另起炉灶写归一化逻辑 | 直接复用 `features::fingerprint(sql)`，在其输出上做 `to_lowercase()` |
| `Config::validate_and_compile()` | 新 config 段未添加到 validate 链路 | 在 `validate_and_compile()` 中添加对 `template_stats` / `chart` 的校验分支 |
| `BufWriter` + SVG 写出 | drop 时不 flush 导致末尾截断 | 每个写出函数末尾显式 `flush()?` |
| `LogProcessor` trait (`&self`) | 统计处理器实现此 trait 但需要 `&mut self` | 使用独立 visitor 接口（`&mut self`），不混入 pipeline |

---

## Performance Traps

| Trap | Symptoms | Prevention | Threshold |
|------|----------|------------|-----------|
| 统计处理器进管线，无统计配置时也执行 | `cargo criterion` 无过滤基准退化 >5% | `pipeline.is_empty()` 守卫 | D-G1: >5% 触发调查 |
| `Vec<u64>` per template 全量样本 | RSS 超 500 MB，1M 记录 | 固定桶直方图，每模板 <300 bytes | 5M 记录 × 1 模板 = 40 MB 单 Vec |
| `fingerprint()` 每条记录调用一次 | CPU 热点在 fingerprint 函数 | 只对有 tag 的 DML 记录调用；PARAMS 记录跳过 | 取决于 SQL 长度，通常 <1μs |
| `HashMap<String, _>` 用 std hasher | hash 速度比 ahash 慢 ~2x | 全项目已用 `ahash::HashMap`，统计器跟进 | 10K 模板时感知不明显；1M 独立 SQL 时明显 |
| SVG 生成中 `format!` 拼接大字符串 | finalize 阶段内存峰值 | `write!` 直接到 `BufWriter` | Top 50 条形图 SVG ~50KB，一次性拼接无害 |

---

## Memory Pitfall: Concrete Numbers

**场景：1.1GB 真实日志文件，~1.55M records/sec，假设 5M 总记录**

| 统计策略 | 假设模板分布 | 内存占用 | 可接受？ |
|---------|------------|---------|---------|
| `Vec<u64>` 全量存储 | 100 模板 × 50K 记录 | 100 × 50K × 8 = 40 MB（仅 Vec 内容，含 HashMap overhead 约 120 MB） | 勉强，但随数据量线性增长 |
| `Vec<u64>` 全量存储 | 1 热模板 × 5M 记录 | 5M × 8 = 40 MB（单 Vec） | 在设备内存有限时危险 |
| `Vec<u64>` 全量存储 | 10K 不同模板 × 500 记录 | 10K × 500 × 8 = 40 MB + HashMap 开销 ~100 MB | 总计 140 MB，设计承诺的"恒定内存"被打破 |
| **64 桶直方图** | 任意分布 | 每模板 288 bytes；10K 模板 = 2.8 MB | **推荐，内存安全** |
| **t-digest** | 任意分布 | 每模板 ~1KB（100 centroid）；10K 模板 = 10 MB | 可接受，但实现复杂度高 |

结论：**`Vec<u64>` 方案在生产数据上不可行**，必须选择固定内存的近似方案。

---

## Existing Test Suite Protection

当前 729 个测试的覆盖范围与新特性的交集：

| 测试类别 | 涉及测试数（估计） | 新特性可能破坏的方式 | 防护措施 |
|---------|----------------|-------------------|---------|
| `config.rs` serde 测试 | ~30 | `FeaturesConfig` 新增字段未加 `#[serde(default)]` 导致反序列化失败 | 新字段用 `Option<T>` |
| `features/mod.rs` Pipeline 测试 | ~15 | `pipeline.is_empty()` 行为改变 | 统计器走独立 visitor 路径 |
| `cli/run.rs` 集成测试 | ~5 | `handle_run` 签名或行为改变 | 统计器参数通过 config 传递，不改 handle_run 签名 |
| `exporter/csv.rs` + `sqlite.rs` | ~50 | finalize 逻辑改变导致文件截断 | 统计写出在 finalize 之后，独立步骤 |
| `features/replace_parameters.rs` | ~40 | `compute_normalized` 被修改以支持模板 key 生成 | 模板 key 复用 `fingerprint()`，不修改 `compute_normalized` |
| `features/sql_fingerprint.rs` | ~10 | `fingerprint()` 函数语义改变 | 如需扩展，添加新函数而非修改现有语义 |

**规则：所有 v1.3 阶段的 exit criteria 必须包含 `cargo test` 729 测试全部通过。**

---

## Phase-Specific Warnings

| Phase | Topic | Most Likely Pitfall | Mitigation |
|-------|-------|---------------------|------------|
| TMPL-01 | SQL 归一化 key 生成 | P3: 另起炉灶 / P4: IN 列表边界 | 先审查 `fingerprint()` 是否满足，缺什么在 `sql_fingerprint.rs` 中扩展 |
| TMPL-01 | Config 结构体 | P8: serde 向后兼容 / P14: apply_overrides | 新字段 `Option<T>` + serde default；同 commit 更新 `apply_one` |
| TMPL-02 | 统计累积器设计 | P1: 破坏快路径 / P2: OOM / P12: trait 冲突 | 先确定接口（RecordVisitor）再写实现；直方图方案 |
| TMPL-02 | 并行路径 | P13: 线程竞争 | 线程独立统计 + 主线程 merge |
| TMPL-03/04 | 统计输出时序 | P7: 与 exporter 生命周期不同步 | finalize 顺序：主 exporter → 统计写出 → SVG 生成 |
| CHART-01~05 | SVG 生成 | P6: 热循环混入 / P11: BufWriter flush | SVG 只在 finalize；显式 flush；BufWriter 直写 |

---

## Sources

- 直接代码检查：`src/features/mod.rs`（`Pipeline::is_empty()`, `LogProcessor` trait, `&self` 约束）
- 直接代码检查：`src/features/sql_fingerprint.rs`（`fingerprint()` 现有实现与局限）
- 直接代码检查：`src/features/replace_parameters.rs`（`compute_normalized`, `ahash::HashMap`, `CompactString` 模式）
- 直接代码检查：`src/cli/run.rs`（`process_log_file` 热循环, `process_csv_parallel` 并行路径, `pipeline.is_empty()` 快路径使用位置）
- 直接代码检查：`src/exporter/mod.rs`（`ExporterKind`, `finalize()` 生命周期, `BufWriter` 使用模式）
- 直接代码检查：`src/config.rs`（`FeaturesConfig::default()`, `apply_one()` match 模式, `validate_and_compile()` 链路）
- `Cargo.toml`：`ahash`, `compact_str`, `memchr`, `smallvec` 已作为依赖可直接复用
- Rust `BufWriter` 文档：drop 时 flush 错误被静默忽略（官方文档明确说明）
- 近似百分位算法：t-digest (Dunning 2013), DDSketch (Masson 2019)；固定桶直方图插值为常用替代

---
*Pitfalls research for: sqllog2db v1.3 — TMPL-01/02/03/04, CHART-01~05*
*Researched: 2026-05-15*

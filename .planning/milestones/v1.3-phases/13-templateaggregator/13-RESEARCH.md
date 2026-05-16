# Phase 13: TemplateAggregator 流式统计累积器 - Research

**Researched:** 2026-05-16
**Domain:** Rust 流式直方图统计 / hdrhistogram / rayon map-reduce
**Confidence:** HIGH

## Summary

Phase 13 在已有的 Phase 12 归一化引擎之上，实现一个纯累积器结构体 `TemplateAggregator`，通过"侧路径"而非 Pipeline 接入热循环。核心技术难点有三：

1. **hdrhistogram API 正确性**：`Histogram::<u64>::new_with_bounds(1, 60_000_000, 2)` 的构造、`record()` 调用在超界时的错误处理（返回 `RecordError`），以及 `add()` 合并要求两端量程相同（本阶段天然满足，因为所有 histogram 由 `TemplateEntry::new()` 统一构造）。

2. **并行路径 map-reduce**：`process_csv_parallel()` 中每个 rayon task 闭包独立持有 `TemplateAggregator`，通过返回值传回主线程，再由 `reduce`/循环 `merge()` 合并。与现有 `ExporterManager` 的"每任务独立实例 + finalize 后汇总"模式完全对称。

3. **零开销快路径保护**：`_do_template: bool` 占位参数必须替换为 `aggregator: Option<&mut TemplateAggregator>`；`aggregator.is_some()` 判断替代原来的 `_do_template`；禁用时 `process_log_file()` 收到 `None`，热循环内不产生任何分支额外开销（`Option<&mut T>` 的 `if let Some(agg)` 经 LLVM 优化为单次空指针检查）。

**Primary recommendation:** 先实现 `src/features/template_aggregator.rs`（含 `TemplateEntry` + `TemplateAggregator` + `TemplateStats`），再修改 `src/cli/run.rs` 的两处集成点（`process_log_file` 签名 + `process_csv_parallel` 任务闭包），最后在 `handle_run()` 中创建/finalize aggregator。

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D-01: hdrhistogram 存储单位为微秒 (µs)，转换：`(pm.exectime * 1000.0) as u64`
- D-02: `TemplateStats` 耗时字段命名：`avg_us`、`min_us`、`max_us`、`p50_us`、`p95_us`、`p99_us`
- D-03: `Histogram::<u64>::new_with_bounds(1, 60_000_000, 2)` — 量程 1µs–60s，sigfig=2（~24 KB/模板）
- D-04: `first_seen`/`last_seen` 类型为 `String`，直接 clone `sqllog.ts.as_ref()`
- D-05: `merge()` 时以字典序比较：`first_seen = min(a, b)`，`last_seen = max(a, b)`（ISO 8601 字典序与时间顺序一致）
- D-06: 复用 `template_analysis.enabled` 同时控制归一化和聚合
- D-07: `enabled = false` 时 `TemplateAggregator` 不创建，收到 `None`，行为与 v1.2 完全一致
- D-08: 本阶段不需要验证"aggregate=true 但 enabled=false"的场景（仅单 enabled 字段）
- D-09: `TemplateAggregator` 不实现 `LogProcessor` trait
- D-10: 内部使用 `hdrhistogram::Histogram<u64>`，禁止 `Vec<u64>` 全量样本存储
- D-11: `observe()` 接收已归一化 key（`normalize_template()` 的输出），不在内部重复归一化
- D-12: 并行 CSV 路径：每 rayon task 持有独立 `TemplateAggregator`，主线程通过 `merge()` 合并

### Claude's Discretion
- `TemplateAggregator` 代码放置：新建 `src/features/template_aggregator.rs`（Phase 14 更容易导入）
- 内部 HashMap 类型：`ahash::AHashMap<String, TemplateEntry>`（项目已依赖 `ahash`）
- `TemplateStats` 添加 `#[derive(Debug, Clone, serde::Serialize)]`（Phase 14 需要序列化）
- `hdrhistogram` 版本：添加 7.5.4（当前 crates.io 最新版，cargo search 确认）
- `finalize()` 返回 `Vec<TemplateStats>`，按 `count desc` 排序（最频繁优先）

### Deferred Ideas (OUT OF SCOPE)
- `TemplateAnalysisConfig` 后续字段（`top_n: usize` 等）— 推迟到 Phase 14/15
- 独立 JSON/CSV 报告输出（TMPL-03/TMPL-03b）— Future Requirements v1.4+
- 单独的 `aggregate: bool` 字段（未来按需添加）
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TMPL-02 | 用户可启用模板统计聚合，run 结束后每个模板输出 count + avg/min/max + p50/p95/p99 + first_seen/last_seen；使用 hdrhistogram（~24 KB/模板），禁止 Vec 全量样本存储 | hdrhistogram 7.5.4 API 已确认（record/value_at_quantile/add/mean/min/max/len），AHashMap 已在项目中，侧路径集成点已定位 |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 耗时样本累积（observe） | 业务逻辑层（features/） | — | 纯内存数据结构，不涉及 I/O；与其他 feature 模块同级 |
| 百分位计算（finalize） | 业务逻辑层（features/） | — | 对 hdrhistogram 的封装，只产生数据，不负责输出 |
| 并行合并（merge） | 业务逻辑层（features/） | CLI 编排层（cli/run.rs） | 合并逻辑在 features 层定义；run.rs 负责 reduce 时序编排 |
| 侧路径接入热循环 | CLI 编排层（cli/run.rs） | — | process_log_file 参数签名变更，热循环内调用 observe() |
| 配置读取 | CLI 编排层（cli/run.rs） | config.rs | do_template 标志已在 handle_run() 计算，Phase 13 将其转为 Option<&mut TA> |
| 输出到文件/数据库 | **NOT Phase 13** | Phase 14 | Phase 13 仅产生 Vec<TemplateStats>，不负责写盘 |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| hdrhistogram | 7.5.4 | 流式百分位直方图 | [VERIFIED: cargo search] 官方 HDR Histogram Rust 移植；~24KB/实例固定内存，误差 <2%；`add()` 支持并行合并 |
| ahash | 0.8 | HashMap 后端（AHashMap） | [VERIFIED: Cargo.toml] 项目已依赖；比 std HashMap 快约 2x 用于字符串 key |
| serde | 1.0.228 | TemplateStats 序列化 | [VERIFIED: Cargo.toml] 项目已依赖；Phase 14 exporter 需要 Serialize |

### Supporting
| Library | Purpose | When to Use |
|---------|---------|-------------|
| rayon（已依赖） | 并行 CSV 任务编排 | process_csv_parallel 中每 task 独立 TemplateAggregator |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| hdrhistogram | t-digest | t-digest 误差更低但 merge 精度损失大；hdrhistogram add() 无精度损失，适合并行路径 |
| hdrhistogram | Vec<u64> + 排序 | Vec 全量存储在 5M 记录/热模板场景下达 40MB+；D-10 明确禁止 |

**Installation:**
```toml
# 在 Cargo.toml [dependencies] 添加：
hdrhistogram = "7.5.4"
```

**Version verification:**
```
cargo search hdrhistogram
→ hdrhistogram = "7.5.4"   # A port of HdrHistogram to Rust
```

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| hdrhistogram | crates.io | ~8 yrs | 85M+ all-time [CITED: crates.io/crates/hdrhistogram] | github.com/HdrHistogram/HdrHistogram_rust | slopcheck unavailable | [ASSUMED] — 高置信度合法（Jon Gjengset 维护，被 tokio-tower 等主流 crate 依赖） |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*slopcheck 在研究环境不可安装，hdrhistogram 标记为 [ASSUMED]。但该包：(1) cargo search 确认存在于 crates.io；(2) 被 tower crate 的 declared_features 列表收录（项目 Cargo.lock 的 target/ 目录已有痕迹）；(3) 由已知 Rust 社区维护者（Jon Gjengset）维护。计划阶段不需要额外 checkpoint，但实现者在 cargo add 后应确认 `cargo tree | grep hdrhistogram` 输出正常。*

## Architecture Patterns

### System Architecture Diagram

```
process_log_file(aggregator: Option<&mut TemplateAggregator>)
         │
         ├─ aggregator.is_none() → 零开销快路径（与 v1.2 完全一致）
         │
         └─ aggregator = Some(agg) → 热循环内：
              ├── normalize_template(pm.sql.as_ref()) → tmpl_key: String
              └── agg.observe(&tmpl_key, exectime_us)
                       │
                       └── AHashMap<String, TemplateEntry>
                                │
                                ├── TemplateEntry.histogram.record(exectime_us)
                                ├── TemplateEntry.count += 1
                                ├── TemplateEntry.first_seen = min(first_seen, ts)
                                └── TemplateEntry.last_seen = max(last_seen, ts)

finalize() → sort by count desc → Vec<TemplateStats>

merge(other: TemplateAggregator):
    for each (key, entry) in other:
        self.entry.histogram.add(other.histogram)   // hdrhistogram add()
        self.entry.count += other.count
        self.entry.first_seen = min(self, other)    // lexicographic
        self.entry.last_seen  = max(self, other)

process_csv_parallel():
    rayon tasks → each task: let mut agg = TemplateAggregator::new()
                              process_log_file(..., Some(&mut agg))
                              returns agg
    main thread → results.into_iter().reduce(|a, b| { a.merge(b); a })
```

### Recommended Project Structure
```
src/
├── features/
│   ├── mod.rs                    # 新增 pub mod template_aggregator; pub use template_aggregator::TemplateAggregator;
│   ├── template_aggregator.rs    # Phase 13 新建：TemplateEntry + TemplateAggregator + TemplateStats
│   ├── sql_fingerprint.rs        # Phase 12 已实现 normalize_template()（去掉 #[allow(dead_code)]）
│   └── ...
├── cli/
│   └── run.rs                    # 修改 process_log_file 签名 + process_csv_parallel + handle_run
```

### Pattern 1: TemplateEntry 内部结构
**What:** 每个模板 key 对应一个 TemplateEntry，持有 histogram + first/last_seen
**When to use:** observe() 的 `entry().or_insert_with()` 访问模式

```rust
// Source: hdrhistogram docs.rs 7.5.4
use hdrhistogram::Histogram;

struct TemplateEntry {
    histogram: Histogram<u64>,
    first_seen: String,
    last_seen: String,
}

impl TemplateEntry {
    fn new(first_seen: String) -> Self {
        // D-03: bounds(1, 60_000_000, 2) = 1µs–60s, sigfig=2
        let histogram = Histogram::<u64>::new_with_bounds(1, 60_000_000, 2)
            .expect("valid bounds");
        Self {
            histogram,
            first_seen: first_seen.clone(),
            last_seen: first_seen,
        }
    }
}
```

### Pattern 2: observe() 热路径
**What:** 用 `&str` 避免每次分配；`entry().or_insert_with()` 懒建 TemplateEntry

```rust
// Source: ahash docs + hdrhistogram record()
pub fn observe(&mut self, key: &str, exectime_us: u64, ts: &str) {
    let entry = self.entries.entry(key.to_string())
        .or_insert_with(|| TemplateEntry::new(ts.to_string()));
    // record() 超界时静默截断至 max_value（不 panic）
    let _ = entry.histogram.record(exectime_us);
    // first_seen: 仅在 < 当前值时更新（初始化时已设置）
    if ts < entry.first_seen.as_str() {
        entry.first_seen = ts.to_string();
    }
    if ts > entry.last_seen.as_str() {
        entry.last_seen = ts.to_string();
    }
}
```

### Pattern 3: merge() 合并
**What:** hdrhistogram `add()` 要求量程相同（本阶段天然满足）

```rust
// Source: hdrhistogram Histogram::add() docs
pub fn merge(&mut self, other: TemplateAggregator) {
    for (key, other_entry) in other.entries {
        let entry = self.entries.entry(key)
            .or_insert_with(|| TemplateEntry::new(other_entry.first_seen.clone()));
        let _ = entry.histogram.add(&other_entry.histogram); // add() 量程相同时 Ok
        if other_entry.first_seen < entry.first_seen {
            entry.first_seen = other_entry.first_seen;
        }
        if other_entry.last_seen > entry.last_seen {
            entry.last_seen = other_entry.last_seen;
        }
    }
}
```

### Pattern 4: finalize() 输出
**What:** 计算百分位，生成 TemplateStats，按 count desc 排序

```rust
// Source: hdrhistogram value_at_quantile() docs
pub fn finalize(self) -> Vec<TemplateStats> {
    let mut stats: Vec<TemplateStats> = self.entries
        .into_iter()
        .map(|(key, entry)| {
            let h = &entry.histogram;
            let count = h.len(); // u64
            TemplateStats {
                template_key: key,
                count,
                avg_us: h.mean() as u64,
                min_us: h.min(),
                max_us: h.max(),
                p50_us: h.value_at_quantile(0.50),
                p95_us: h.value_at_quantile(0.95),
                p99_us: h.value_at_quantile(0.99),
                first_seen: entry.first_seen,
                last_seen: entry.last_seen,
            }
        })
        .collect();
    stats.sort_unstable_by(|a, b| b.count.cmp(&a.count));
    stats
}
```

### Pattern 5: process_log_file 签名替换
**What:** 用 `aggregator: Option<&mut TemplateAggregator>` 替换 `_do_template: bool`

```rust
fn process_log_file(
    // ... 其他参数不变 ...
    do_normalize: bool,
    aggregator: Option<&mut TemplateAggregator>,  // 替换 _do_template: bool
    placeholder_override: Option<bool>,
    // ...
) -> Result<usize> {
    // 热循环内：
    if let Some(agg) = aggregator.as_deref_mut() {
        let tmpl_key = crate::features::normalize_template(pm.sql.as_ref());
        agg.observe(&tmpl_key, (pm.exectime * 1000.0) as u64, record.ts.as_ref());
    }
}
```

### Pattern 6: process_csv_parallel 中的 map-reduce
**What:** 每任务独立 aggregator，返回给主线程合并

```rust
// 在任务闭包内
let mut task_agg = TemplateAggregator::new();
let count = process_log_file(..., Some(&mut task_agg), ...)?;
Ok(Some((file.clone(), temp_path, count, task_agg)))

// 主线程收集后
let merged_agg = task_aggs.into_iter().reduce(|mut a, b| { a.merge(b); a });
```

### Anti-Patterns to Avoid
- **实现 LogProcessor trait**：`process()` 接收 `&self`，累积需要 `&mut self`；加入 Pipeline 会破坏 `pipeline.is_empty()` 快路径（D-09 明确禁止）
- **Vec<u64> 全量存储**：5M 记录/热模板达 40MB，多模板叠加超 200MB（D-10 明确禁止）
- **在 observe() 内重复归一化**：key 应已由调用方归一化，避免双重开销（D-11）
- **共享 Mutex<TemplateAggregator>**：在 rayon 任务间共享锁会造成竞争；正确方案是每任务独立实例 + merge（D-12）
- **histogram.record() panic on error**：record() 返回 Result；超界时应静默忽略（`let _ = entry.histogram.record(...)`），或用 `record_corrected()` 截断

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 百分位计算 | sort + index 或 running percentile | hdrhistogram::Histogram::value_at_quantile() | sort O(n log n) 且需全量存储；running percentile 精度差；hdrhistogram O(1) 查询，~24KB 固定内存 |
| 流式 mean/min/max | 手写累积变量 | hdrhistogram::Histogram::mean()/min()/max() | histogram 的 mean 比分别维护变量一致性更好（特别是 merge 时） |
| parallel histogram merge | Arc<Mutex<Histogram>> | hdrhistogram::Histogram::add() + reduce | add() 无锁，精度无损失，两直方图量程相同时 O(bucket_count) |

**Key insight:** hdrhistogram 是为服务器延迟监控设计的，它的 merge 语义（add()）精确无近似，是并行路径下的正确选择。

## Common Pitfalls

### Pitfall 1: histogram.record() 超界不 panic 但静默截断
**What goes wrong:** `record(exectime_us)` 在 exectime_us > 60_000_000 时返回 `Err(RecordError::ValueOutOfRangeResizeDisabled)`，如果 `?` 传播会导致整个文件处理中断
**Why it happens:** hdrhistogram 默认不自动 resize；量程设定为 60s 已覆盖正常 SQL，但异常慢查询可能超界
**How to avoid:** 用 `let _ = entry.histogram.record(exectime_us.min(60_000_000));` 先截断，或忽略错误 `let _ = entry.histogram.record(exectime_us);`
**Warning signs:** clippy 警告 `unused Result`（`-D warnings` 下会报错）

### Pitfall 2: process_csv_parallel 中 aggregator 生命周期问题
**What goes wrong:** rayon 任务闭包需要 `'static` 或满足 `Send` bound；`&mut TemplateAggregator` 不能跨线程传递
**Why it happens:** `Option<&mut T>` 不是 `Send`；直接在任务闭包外创建并传入会被 borrow checker 拒绝
**How to avoid:** 每个任务闭包内 `let mut agg = TemplateAggregator::new()`，在闭包内完整生命周期，返回 `agg`（Move 语义）
**Warning signs:** 编译错误 "cannot be sent between threads safely"

### Pitfall 3: merge() 中 or_insert_with 的 first_seen 初始化
**What goes wrong:** 当 `self.entries` 中尚无该 key 时，用 `or_insert_with` 新建 entry 的 `first_seen` 应该是 `other_entry.first_seen`（不是默认空字符串）
**Why it happens:** `TemplateEntry::new()` 需要一个初始 `first_seen` 参数，容易传错
**How to avoid:** `or_insert_with(|| TemplateEntry::new(other_entry.first_seen.clone()))`，之后不再重复赋值 first_seen

### Pitfall 4: `_do_template` 参数重命名引发的调用处编译错误
**What goes wrong:** `process_log_file` 签名从 `_do_template: bool` 变为 `aggregator: Option<&mut TemplateAggregator>`，所有调用处（顺序路径 + 并行路径）必须同步更新
**Why it happens:** 有两处调用：顺序路径 `process_log_file()` 和 `process_csv_parallel()` 内的任务闭包
**How to avoid:** 修改签名后立即 `cargo build`，让编译器列出所有调用处

### Pitfall 5: clippy 对 `allow(dead_code)` 的处理
**What goes wrong:** Phase 12 在 `normalize_template`、`ScanMode::Normalize` 和 `pub use sql_fingerprint::normalize_template` 上加了 `#[allow(dead_code)]`，Phase 13 接入后需要同时移除这些 allow 属性，否则会留下无效注解
**Why it happens:** `-D warnings` 下未使用的 `allow` 属性本身可能触发警告（取决于 Rust 版本）
**How to avoid:** 接入 `observe()` 调用后，搜索并删除 `// Phase 13 will re-enable this` 相关注释和 `allow(dead_code)` 属性

## Code Examples

### 完整的 TemplateAggregator 骨架（待实现参考）

```rust
// src/features/template_aggregator.rs
// Source: hdrhistogram 7.5.4 docs.rs + D-01 ~ D-12 决策

use ahash::AHashMap;
use hdrhistogram::Histogram;
use serde::Serialize;

struct TemplateEntry {
    histogram: Histogram<u64>,
    first_seen: String,
    last_seen: String,
}

impl TemplateEntry {
    fn new(first_seen: String) -> Self {
        let histogram = Histogram::<u64>::new_with_bounds(1, 60_000_000, 2)
            .expect("D-03: valid bounds 1µs–60s sigfig=2");
        Self {
            histogram,
            first_seen: first_seen.clone(),
            last_seen: first_seen,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateStats {
    pub template_key: String,
    pub count: u64,
    pub avg_us: u64,
    pub min_us: u64,
    pub max_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub first_seen: String,  // D-04
    pub last_seen: String,   // D-04
}

#[derive(Debug, Default)]
pub struct TemplateAggregator {
    entries: AHashMap<String, TemplateEntry>,
}

impl TemplateAggregator {
    pub fn new() -> Self { Self::default() }

    pub fn observe(&mut self, key: &str, exectime_us: u64, ts: &str) { ... }
    pub fn merge(&mut self, other: TemplateAggregator) { ... }
    pub fn finalize(self) -> Vec<TemplateStats> { ... }
}
```

### process_log_file 调用点（热循环内）

```rust
// src/cli/run.rs — 热循环内，紧接 compute_normalized 之后
// 替换注释掉的 D-14 占位代码
if let Some(agg) = aggregator.as_deref_mut() {
    if record.tag.is_some() { // 只统计 DML 记录（有 tag），跳过 PARAMS
        let tmpl_key = crate::features::normalize_template(pm.sql.as_ref());
        agg.observe(&tmpl_key, (pm.exectime * 1000.0) as u64, record.ts.as_ref());
    }
}
```

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| `_do_template: bool` 占位参数 | `aggregator: Option<&mut TemplateAggregator>` | 类型安全，None 即零开销 |
| 注释掉的 normalize_template 调用 | observe() 接收 key | Phase 13 解封 `#[allow(dead_code)]` |
| hdrhistogram 不在 Cargo.toml | 需要添加 `hdrhistogram = "7.5.4"` | 新增依赖 |

**Deprecated/outdated:**
- `_do_template: bool` 参数：Phase 13 删除，替换为 `aggregator: Option<&mut TemplateAggregator>`
- `#[allow(dead_code)]` 在 `normalize_template`/`ScanMode::Normalize`：Phase 13 接入后移除

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | hdrhistogram 7.5.4 是 crates.io 最新稳定版 | Standard Stack | 若已有 7.5.5+ 则 Cargo.toml 写死版本号仍可用（semver 兼容），风险极低 |
| A2 | hdrhistogram `record()` 超界返回 Err 而非 panic（无 resize） | Common Pitfalls | 若行为不同需调整错误处理策略，但 docs.rs 文档明确说明 |
| A3 | `histogram.mean()` 返回 `f64`，需 `as u64` 截断 | Code Examples | 若 mean() 签名变化需调整，低风险 |

## Open Questions

1. **是否需要统计 PARAMS 记录（无 tag）**
   - What we know: PARAMS 记录通常不是独立 SQL 语句，而是 DML 的参数补丁
   - What's unclear: CONTEXT.md 未明确说明是否观测 PARAMS 记录的 exectime
   - Recommendation: 仅观测有 `tag` 的 DML 记录（与 sql_record_filter 逻辑一致），忽略 PARAMS；实现者确认后可调整

2. **observe() 中 exectime=0.0 的处理**
   - What we know: `include_performance_metrics=false` 时 exectime 被赋值 0.0（合成空 pm）
   - What's unclear: 0.0 * 1000.0 = 0 as u64 = 0；但 histogram bounds 最小值为 1µs，record(0) 会超界
   - Recommendation: 在 observe() 或调用处 guard：`if exectime_us > 0 { agg.observe(...) }`；或 `record(exectime_us.max(1))`

## Environment Availability

Step 2.6: SKIPPED — Phase 13 是纯 Rust 代码变更，新增 Cargo 依赖（hdrhistogram）；无外部服务或 CLI 工具依赖。`cargo`、`rustc` 已在项目中正常运行（Phase 12 已验证）。

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust 内置 `#[test]` + `cargo test` |
| Config file | 无（Rust 内置，无需配置文件） |
| Quick run command | `cargo test -p dm-database-sqllog2db template_aggregator 2>&1 | tail -20` |
| Full suite command | `cargo test 2>&1 | tail -20` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TMPL-02 | observe() 累积单条记录后 count=1，avg/min/max 正确 | unit | `cargo test test_observe_single` | ❌ Wave 0 |
| TMPL-02 | observe() 多条记录后百分位统计正确（p50/p95/p99） | unit | `cargo test test_finalize_percentiles` | ❌ Wave 0 |
| TMPL-02 | merge() 合并两个 aggregator，统计结果与单线程等价 | unit | `cargo test test_merge_equivalent` | ❌ Wave 0 |
| TMPL-02 | first_seen/last_seen 在 merge 后取正确字典序极值 | unit | `cargo test test_merge_timestamps` | ❌ Wave 0 |
| TMPL-02 | enabled=false 时 process_log_file 收到 None，零开销（编译验证） | unit | `cargo test test_aggregator_disabled_path` | ❌ Wave 0 |
| TMPL-02 | 并行路径 merge 后统计与顺序路径一致（集成测试） | integration | `cargo test test_parallel_merge_consistent` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test template_aggregator`
- **Per wave merge:** `cargo test && cargo clippy --all-targets -- -D warnings`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/features/template_aggregator.rs` — 新建，包含所有单元测试（observe/finalize/merge 路径）
- [ ] `src/features/mod.rs` — 添加 `pub mod template_aggregator;` 和 `pub use template_aggregator::TemplateAggregator;`
- [ ] `Cargo.toml` — 添加 `hdrhistogram = "7.5.4"` 依赖

## Security Domain

Phase 13 无网络 I/O、无用户输入解析、无权限边界操作。纯内存数据结构变更，ASVS 各类别均不适用。

## Sources

### Primary (HIGH confidence)
- [hdrhistogram 7.5.4 docs.rs](https://docs.rs/hdrhistogram/7.5.4/hdrhistogram/struct.Histogram.html) — new_with_bounds, record, value_at_quantile, add, mean, min, max, len API 确认
- `Cargo.toml` — ahash 0.8、serde 1.0.228、rayon 1.11 已有依赖确认 [VERIFIED]
- `src/cli/run.rs` — process_log_file 参数列表、process_csv_parallel 任务闭包结构、handle_run finalize 位置 [VERIFIED]
- `src/features/mod.rs` — pub use 导出模式（`pub use sql_fingerprint::fingerprint`） [VERIFIED]
- `src/features/sql_fingerprint.rs` — normalize_template() 函数签名 `fn normalize_template(sql: &str) -> String` [VERIFIED]
- `.planning/phases/13-templateaggregator/13-CONTEXT.md` — D-01 ~ D-12 锁定决策 [VERIFIED]

### Secondary (MEDIUM confidence)
- [crates.io hdrhistogram](https://crates.io/crates/hdrhistogram) — 版本 7.5.4、下载量 85M+、Jon Gjengset 维护

### Tertiary (LOW confidence)
- hdrhistogram record() 超界行为（`RecordError::ValueOutOfRangeResizeDisabled`）— 基于训练知识 [ASSUMED]，docs.rs 页面已通过 WebFetch 确认 API 存在但错误类型名称未逐字验证

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — hdrhistogram 7.5.4 cargo search 确认；ahash/serde/rayon 已在 Cargo.toml
- Architecture: HIGH — 代码已仔细阅读，集成点、签名变更位置均已定位
- Pitfalls: MEDIUM — clippy 相关行为基于项目已有 `-D warnings` 配置推断；hdrhistogram 超界行为基于 docs.rs

**Research date:** 2026-05-16
**Valid until:** 2026-06-16（hdrhistogram API 稳定，30 天内无需重验证）

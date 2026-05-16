# Phase 13: TemplateAggregator 流式统计累积器 - Pattern Map

**Mapped:** 2026-05-16
**Files analyzed:** 5
**Analogs found:** 5 / 5

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/features/template_aggregator.rs` | service / accumulator | streaming, batch | `src/features/sql_fingerprint.rs` (结构) + `src/exporter/mod.rs` (finalize 生命周期) | role-match |
| `src/features/mod.rs` | module root | — | `src/features/mod.rs` 自身（现有 pub use 模式） | exact |
| `src/cli/run.rs` | orchestration | streaming, request-response | `src/cli/run.rs` 自身（process_log_file + process_csv_parallel） | exact |
| `src/config.rs` | config | — | `src/config.rs` 自身（apply_one 模式已完备，Phase 13 无需修改） | exact |
| `Cargo.toml` | config | — | `Cargo.toml` 自身（现有依赖区块） | exact |

---

## Pattern Assignments

### `src/features/template_aggregator.rs` (service, streaming/batch)

**主要参照：** `src/features/sql_fingerprint.rs`（同目录 feature 模块结构）
**生命周期参照：** `src/exporter/mod.rs` 的 `finalize()` 模式

**Imports 模式** — 参照项目已有依赖（`ahash`、`serde` 已在 Cargo.toml）：
```rust
use ahash::AHashMap;
use hdrhistogram::Histogram;
use serde::Serialize;
```

**模块内私有结构 + 公开结构模式** — 参照 `src/features/sql_fingerprint.rs` 第 23-46 行（私有 `ScanMode` + 公开 `fingerprint()`/`normalize_template()`）：
```rust
// 私有内部状态，不 pub
struct TemplateEntry {
    histogram: Histogram<u64>,
    first_seen: String,
    last_seen: String,
}

// 公开输出类型，供 Phase 14 序列化
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
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Debug, Default)]
pub struct TemplateAggregator {
    entries: AHashMap<String, TemplateEntry>,
}
```

**TemplateEntry::new() 构造** — D-03 量程规范，参照 RESEARCH.md Pattern 1：
```rust
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
```

**observe() 热路径** — D-11：key 已由调用方归一化；D-04：ts 直接 clone；参照 RESEARCH.md Pattern 2。
关键：record 超界时 `let _ = ...` 静默忽略（CLAUDE.md `-D warnings` 强制，不能留 unused Result）：
```rust
pub fn observe(&mut self, key: &str, exectime_us: u64, ts: &str) {
    let entry = self.entries.entry(key.to_string())
        .or_insert_with(|| TemplateEntry::new(ts.to_string()));
    let _ = entry.histogram.record(exectime_us);
    if ts < entry.first_seen.as_str() {
        entry.first_seen = ts.to_string();
    }
    if ts > entry.last_seen.as_str() {
        entry.last_seen = ts.to_string();
    }
}
```

**merge() 并行合并** — D-05 字典序；hdrhistogram `add()` 无精度损失；参照 RESEARCH.md Pattern 3：
```rust
pub fn merge(&mut self, other: TemplateAggregator) {
    for (key, other_entry) in other.entries {
        let entry = self.entries.entry(key)
            .or_insert_with(|| TemplateEntry::new(other_entry.first_seen.clone()));
        let _ = entry.histogram.add(&other_entry.histogram);
        if other_entry.first_seen < entry.first_seen {
            entry.first_seen = other_entry.first_seen;
        }
        if other_entry.last_seen > entry.last_seen {
            entry.last_seen = other_entry.last_seen;
        }
    }
}
```

**finalize() 输出** — 按 count desc 排序；参照 RESEARCH.md Pattern 4：
```rust
pub fn finalize(self) -> Vec<TemplateStats> {
    let mut stats: Vec<TemplateStats> = self.entries
        .into_iter()
        .map(|(key, entry)| {
            let h = &entry.histogram;
            TemplateStats {
                template_key: key,
                count: h.len(),
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

**单元测试模式** — 参照 `src/features/sql_fingerprint.rs` 第 329-439 行（同文件底部 `#[cfg(test)] mod tests`）：
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observe_single() { ... }

    #[test]
    fn test_finalize_percentiles() { ... }

    #[test]
    fn test_merge_equivalent() { ... }

    #[test]
    fn test_merge_timestamps() { ... }
}
```

---

### `src/features/mod.rs` (module root)

**参照：** `src/features/mod.rs` 自身第 1-11 行（现有 pub mod + pub use 模式）

**现有 pub use 导出模式**（lines 4-11）：
```rust
pub mod replace_parameters;
pub use replace_parameters::compute_normalized;

pub mod sql_fingerprint;
pub use sql_fingerprint::fingerprint;
// Phase 13 will re-enable this when TemplateAggregator::observe() is wired in.
#[allow(unused_imports)]
pub use sql_fingerprint::normalize_template;
```

**Phase 13 新增，遵循同一模式**（在 `pub mod sql_fingerprint;` 块之后插入）：
```rust
pub mod template_aggregator;
pub use template_aggregator::TemplateAggregator;
```

**同时移除的内容**（Phase 12 留下的占位注释和 allow 属性）：
- 删除 `// Phase 13 will re-enable this when TemplateAggregator::observe() is wired in.`
- 删除 `#[allow(unused_imports)]`（`normalize_template` 接入后不再是 dead import）

**sql_fingerprint.rs 中同步移除**（lines 25-27 和 lines 43-44）：
```rust
// 删除：
#[allow(dead_code)]
// Normalize variant 上的
#[allow(dead_code)]
// normalize_template 函数上的
```

---

### `src/cli/run.rs` (orchestration, streaming)

**参照：** 自身，定位三处修改点

#### 修改点 1：`process_log_file` 函数签名（lines 114-130）

**当前签名**（line 124-125，需替换）：
```rust
    do_normalize: bool,
    _do_template: bool,        // ← 删除此行
```

**替换为**（参照 RESEARCH.md Pattern 5）：
```rust
    do_normalize: bool,
    aggregator: Option<&mut crate::features::TemplateAggregator>,  // ← 新增
```

#### 修改点 2：`process_log_file` 热循环内（lines 222-228，当前注释掉的占位代码）

**当前注释块**（lines 222-228）：
```rust
                            // D-14: Phase 13 will wire this into TemplateAggregator::observe().
                            // let _tmpl_key = if do_template {
                            //     Some(crate::features::normalize_template(pm.sql.as_ref()))
                            // } else {
                            //     None
                            // };
```

**替换为**（参照 RESEARCH.md Pattern 5 + Code Examples 热循环调用点）：
```rust
                            // 模板聚合侧路径：仅对有 tag 的 DML 记录统计（跳过 PARAMS）
                            if let Some(agg) = aggregator.as_deref_mut() {
                                if record.tag.is_some() {
                                    let tmpl_key = crate::features::normalize_template(pm.sql.as_ref());
                                    let exectime_us = (pm.exectime * 1000.0) as u64;
                                    if exectime_us > 0 {
                                        agg.observe(&tmpl_key, exectime_us, record.ts.as_ref());
                                    }
                                }
                            }
```

注：`exectime_us > 0` 防止 `include_performance_metrics=false` 时合成的 `exectime=0.0` 超出 histogram 最小界 1µs（参照 RESEARCH.md Open Questions Q2）。

#### 修改点 3：`process_csv_parallel` 函数（lines 451-620）

**函数签名** — `do_template: bool` 参数替换为 `aggregator_enabled: bool`（或直接去掉，内部判断），返回类型扩展携带 `Option<TemplateAggregator>`：

实际上，并行路径更干净的设计是：签名接收 `aggregator_enabled: bool`，每个任务内部 `TemplateAggregator::new()`，任务返回类型从 `Option<(PathBuf, PathBuf, usize)>` 扩展为 `Option<(PathBuf, PathBuf, usize, Option<TemplateAggregator>)>`。

**任务闭包内**（参照 RESEARCH.md Pattern 6，在 `process_log_file` 调用前后）：
```rust
// 任务闭包内（原 do_template 参数使用处）
let mut task_agg = if do_template {
    Some(crate::features::TemplateAggregator::new())
} else {
    None
};

let count = process_log_file(
    /* ... 其他参数不变 ... */
    task_agg.as_mut(),   // Option<&mut TemplateAggregator>
    /* ... */
)?;

Ok(Some((file.clone(), temp_path, count, task_agg)))
```

**主线程 reduce 合并**（在 `parts_info` 收集循环之后）：
```rust
let merged_agg: Option<crate::features::TemplateAggregator> = parts_info
    .iter_mut()
    .filter_map(|(_, _, _, agg)| agg.take())
    .reduce(|mut a, b| { a.merge(b); a });
```

#### 修改点 4：`handle_run` 函数 — 创建/传入/finalize aggregator（lines 640+）

**参照：** `exporter_manager.finalize()` 调用模式（line 853）

`do_template` 标志已在 lines 716-718 计算：
```rust
let do_template = final_cfg
    .features
    .template_analysis
    .as_ref()
    .is_some_and(|t| t.enabled);
```

在此之后创建 aggregator：
```rust
let mut aggregator = if do_template {
    Some(crate::features::TemplateAggregator::new())
} else {
    None
};
```

顺序路径 `process_log_file` 调用处（line 822 附近），替换 `do_template` 参数：
```rust
    aggregator.as_mut(),   // 替换原来的 do_template
```

并行路径调用 `process_csv_parallel` 后，从返回值中取出 `merged_agg` 并赋给 `aggregator`。

finalize（在 `exporter_manager.finalize()?;` 同位置附近）：
```rust
let template_stats = aggregator.map(|agg| agg.finalize());
// Phase 14 将消费 template_stats；Phase 13 暂时用 log 记录数量
if let Some(ref stats) = template_stats {
    info!("Template analysis: {} unique templates", stats.len());
}
```

---

### `Cargo.toml` (config)

**参照：** `Cargo.toml` 自身 `[dependencies]` 区块（lines 32-69）

在 `ahash = "0.8"` 行附近（相关依赖聚集）新增：
```toml
hdrhistogram = "7.5.4"
```

验证：`cargo tree | grep hdrhistogram` 应输出 `hdrhistogram v7.5.4`。

---

### `src/config.rs` (config)

**结论：Phase 13 无需修改 config.rs。**

`features.template_analysis.enabled` 的 `apply_one()` 处理已在 Phase 12 实现（lines 255-260）：
```rust
"features.template_analysis.enabled" => {
    self.features
        .template_analysis
        .get_or_insert_with(Default::default)
        .enabled = parse_bool(value)?;
}
```

`handle_run()` 中已通过 `t.enabled` 读取配置（lines 716-718），无需新增字段。

---

## Shared Patterns

### `#[allow(dead_code)]` 清理
**来源：** `src/features/sql_fingerprint.rs` lines 25-27, 43-44；`src/features/mod.rs` lines 9-11
**适用范围：** Phase 13 接入 `normalize_template` 调用后，以下 allow 属性必须删除：
- `sql_fingerprint.rs` line 26: `#[allow(dead_code)]`（`ScanMode::Normalize` variant）
- `sql_fingerprint.rs` line 43: `#[allow(dead_code)]`（`normalize_template` 函数）
- `mod.rs` lines 9-10: 注释 + `#[allow(unused_imports)]`（`pub use sql_fingerprint::normalize_template`）

清理方法：接入 `observe()` 调用后执行 `cargo clippy --all-targets -- -D warnings`，编译器会报告残留的无效 allow 属性（部分 Rust 版本会对 `unused_attributes` 发出警告）。

### 侧路径 Option<&mut T> 模式
**来源：** `src/cli/run.rs` lines 207-220（`do_normalize` 条件守卫的现有模式）
**适用范围：** `process_log_file` 热循环内对 `aggregator` 的调用
```rust
// 现有模式（do_normalize 条件守卫）：
let ns = if do_normalize && ... {
    crate::features::compute_normalized(...)
} else {
    None
};

// 新增模式（aggregator 侧路径守卫），结构完全对称：
if let Some(agg) = aggregator.as_deref_mut() {
    // 仅在 aggregator 存在时执行，None 时 LLVM 优化为单次空指针检查
}
```

### finalize 生命周期
**来源：** `src/cli/run.rs` line 853（`exporter_manager.finalize()?;`）和 line 566（并行路径 `em.finalize()?;`）
**适用范围：** `aggregator.finalize()` 必须在以下时机调用：
- 顺序路径：`exporter_manager.finalize()?;` 之后（handle_run 末尾）
- 并行路径：所有任务完成后的 reduce 阶段，主线程调用

### clippy `-D warnings` 合规
**来源：** `Cargo.toml` lints 配置（lines 71-87）；`CLAUDE.md` 明确要求
**适用范围：** 所有新代码
- `let _ = entry.histogram.record(...)` — 不能省略（unused Result → error）
- `let _ = entry.histogram.add(...)` — 同上
- `sort_unstable_by` 优于 `sort_by`（clippy `stable_sort_primitive` lint）
- 函数超 40 行需拆分（CLAUDE.md 规范）

---

## No Analog Found

无——所有文件均在项目中找到了直接参照。

---

## Metadata

**Analog search scope:** `src/features/`, `src/cli/`, `src/exporter/`, `src/config.rs`, `Cargo.toml`
**Files scanned:** 6
**Pattern extraction date:** 2026-05-16

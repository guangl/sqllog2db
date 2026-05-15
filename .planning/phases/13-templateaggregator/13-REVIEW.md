---
phase: 13-templateaggregator
reviewed: 2026-05-16T00:00:00Z
depth: standard
files_reviewed: 3
files_reviewed_list:
  - src/features/mod.rs
  - src/features/sql_fingerprint.rs
  - src/cli/run.rs
findings:
  critical: 2
  warning: 3
  info: 2
  total: 7
status: fixed
fixed_at: 2026-05-16T00:00:00Z
fixes_applied:
  - CR-01: da9a36a
  - CR-02: f91062c
  - WR-01: 97405bb
  - WR-02: 95f448c
  - WR-03: 8dba975
---

# Phase 13: Code Review Report

**Reviewed:** 2026-05-16
**Depth:** standard
**Files Reviewed:** 3
**Status:** issues_found

## Summary

代码审查范围：`TemplateAggregator` 模块（`template_aggregator.rs`，通过 `mod.rs` 引入）、SQL 指纹/归一化模块（`sql_fingerprint.rs`），以及把两者接入热循环的 `cli/run.rs`。

总体质量较高：`hdrhistogram` 使用方式正确，IN 列表折叠逻辑健全，UTF-8 安全性有保证，并行 map-reduce 路径实现干净。

发现两个 BLOCKER：一个是当 `include_performance_metrics=false` 时模板统计全部失效（exectime 永远为 0us，落在 histogram 下限之外被静默丢弃），另一个是在带 `--limit` 的顺序路径中模板计数比实际导出的记录多一条。此外还有三个 WARNING 和两个 INFO 项。

---

## Critical Issues

### CR-01: template_analysis + include_performance_metrics=false 导致统计全部归零

**File:** `src/cli/run.rs:189-234`

**Issue:**
当 CSV 导出器配置 `include_performance_metrics = false` 时，`process_log_file` 用合成的 `PerformanceMetrics { exectime: 0.0, ... }` 替代真实的 `parse_performance_metrics()`（L189-198）。随后在 L233，`exectime_us = (0.0 * 1000.0) as u64 = 0`，调用 `agg.observe(key, 0, ts)`。

`TemplateEntry` 的 histogram 下限为 `1`（`new_with_bounds(1, 60_000_000, 2)`），`Histogram::record(0)` 返回 `Err` 并被 `let _` 静默丢弃。所有 DML 记录的执行时间均无法写入 histogram，`h.len() = 0`，最终 `TemplateStats.count = 0`，所有百分位字段也全为 0。

用户同时启用 `[features.template_analysis] enabled = true` 和 `include_performance_metrics = false` 时，将获得完全无意义的统计结果，且没有任何警告。

**Fix:**
在 `process_log_file` 中，当 `do_template = true` 时，无论 `include_pm` 如何，都必须调用真实的 `parse_performance_metrics()`（或至少单独调用 `parse_indicators()` 来获取 exectime）。最简修复：仅在 `do_template` 激活时强制解析性能指标：

```rust
// 决定是否需要真实的 pm（用于 template 聚合的 exectime）
let pm = if include_pm || aggregator.is_some() {
    record.parse_performance_metrics()
} else {
    dm_database_parser_sqllog::PerformanceMetrics {
        sql: record.body(),
        exectime: 0.0,
        rowcount: 0,
        exec_id: 0,
    }
};
```

或者，在 `observe()` 中对 0us 做箝位处理（以 1us 记录），并在 `TemplateStats` 中标注数据不可靠，但前者更干净。

---

### CR-02: --limit 路径下模板聚合多计一条记录

**File:** `src/cli/run.rs:224-246`

**Issue:**
`agg.observe()` 在 L234 被调用，之后才是 L239-243 的 limit 检查。当 `records_in_file >= remaining` 时，代码 `break 'outer` 不导出该记录，但 observe 已经执行——模板统计里计了一条实际未导出的记录。对于重复命中该临界点的场景（多文件 + 限量），每文件都会多计一条。

```
L224-235: agg.observe(key, exectime_us, ts)  ← 已计入统计
L239-243: if records_in_file >= remaining { break 'outer }  ← 中止导出
L245:     export_one_preparsed(...)  ← 不会执行
```

**Fix:**
将 limit 检查移到 `observe()` 之前：

```rust
// 先检查配额，再聚合
if let Some(remaining) = limit {
    if records_in_file >= remaining {
        break 'outer;
    }
}

// 模板聚合（仅在导出路径上）
if let Some(ref mut agg) = aggregator {
    if record.tag.is_some() {
        let tmpl_key = crate::features::normalize_template(pm.sql.as_ref());
        let exectime_us = (pm.exectime * 1000.0) as u64;
        agg.observe(&tmpl_key, exectime_us, record.ts.as_ref());
    }
}

exporter_manager.export_one_preparsed(...)?;
records_in_file += 1;
```

---

## Warnings

### WR-01: exectime 超出 histogram 范围时静默丢弃

**File:** `src/features/template_aggregator.rs:67`

**Issue:**
`let _ = entry.histogram.record(exectime_us)` 静默忽略了两种有效场景的错误：

1. `exectime_us = 0`：正常路径下查询耗时 < 1ms（DM 日志 exectime 分辨率为 ms，很多缓存命中查询记为 0ms）。histogram 下限为 `1`，`record(0)` 失败，**这些记录不计入 count**，导致 `TemplateStats.count` 低于实际观测次数。
2. `exectime_us > 60_000_000`（即 >60s 的慢查询）：同样被静默丢弃。

**Fix:**
使用饱和箝位或扩大 histogram 范围：

```rust
// 箝位到 [1, 60_000_000] 保证所有样本都能计入
let clamped = exectime_us.clamp(1, 60_000_000);
let _ = entry.histogram.record(clamped);
```

或直接分开维护 count（独立于 histogram 的 `u64` 计数器），使 `count` 不依赖 `h.len()`。

---

### WR-02: FieldMask::from_names 对空列表返回 FieldMask(0)，与 D-02 决策冲突

**File:** `src/features/mod.rs:45-54` 和 `144-148`

**Issue:**
当用户配置 `features.fields = []`（空列表）时：

- `ordered_field_indices()` 遵循 D-02：空列表等同于 `None`，返回全部 15 个索引（L158）。
- `field_mask()` 调用 `FieldMask::from_names(&[])` → 循环零次 → `mask = 0u16` → 返回 `FieldMask(0)`（非 `ALL`），而非退化到 `ALL`。

`FieldMask(0).includes_normalized_sql()` 返回 `false`，导致 `do_normalize = false`，用户虽然能获得 14 列输出（因为 `ordered_indices` 包含 14，但 normalize=false 时 header/data 均跳过索引 14），但行为与 `fields = None`（15 列）不一致。这违反了 D-02 设计文档。

**Fix:**

```rust
pub fn field_mask(&self) -> FieldMask {
    match &self.fields {
        None => FieldMask::ALL,
        Some(names) if names.is_empty() => FieldMask::ALL, // D-02
        Some(names) => FieldMask::from_names(names).unwrap_or(FieldMask::ALL),
    }
}
```

---

### WR-03: count_placeholders 和 apply_params_into 中的 usize 整数折叠可溢出

**File:** `src/features/replace_parameters.rs:168` 和 `272`

**Issue:**
`:N` 序号占位符的解析使用 fold 累加：
```rust
let n: usize = bytes[start..j]
    .iter()
    .fold(0usize, |acc, &b| acc * 10 + (b - b'0') as usize);
```

对于异常长的序号（如 `:99999999999999999999`，>20 位），`acc * 10` 在 debug 构建（含测试）下会触发 panic，release 构建（wraps）下则得到错误的参数索引，可能导致参数替换错误（不会越界，因为 `params.get(idx)` 返回 `None`，但会静默跳过替换）。

**Fix:**
使用 `saturating` 乘法或加一个长度上限：

```rust
let n: usize = bytes[start..j]
    .iter()
    .fold(0usize, |acc, &b| {
        acc.saturating_mul(10).saturating_add((b - b'0') as usize)
    });
```

---

## Info

### IN-01: FieldMask::is_active 是公开 API 但无外部调用者

**File:** `src/features/mod.rs:58-61`

**Issue:**
`pub fn is_active(self, idx: usize) -> bool` 在代码库中仅被 `includes_normalized_sql()` 调用（同文件内部）。`is_active` 作为公开函数存在于 API 表面，但无外部调用者。

**Fix:**
如无对外暴露 API 的计划，将可见性降为 `pub(crate)` 或 `fn`：

```rust
#[inline]
#[must_use]
fn is_active(self, idx: usize) -> bool {  // 或 pub(crate)
    idx < 15 && (self.0 >> idx) & 1 == 1
}
```

---

### IN-02: finalize() 使用 sort_unstable_by 导致等 count 模板顺序不确定

**File:** `src/features/template_aggregator.rs:122`

**Issue:**
`stats.sort_unstable_by(|a, b| b.count.cmp(&a.count))` 对 count 相等的模板不保证稳定顺序，每次运行可能产生不同排列。当 Phase 14 将结果写入报告文件时，diff 比较或幂等性测试将受影响。

**Fix:**
在次级排序键（如 `template_key`）上添加 tie-breaking：

```rust
stats.sort_unstable_by(|a, b| {
    b.count.cmp(&a.count)
        .then_with(|| a.template_key.cmp(&b.template_key))
});
```

或切换到 `sort_by`（稳定排序），代价是略高的内存使用。

---

_Reviewed: 2026-05-16_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

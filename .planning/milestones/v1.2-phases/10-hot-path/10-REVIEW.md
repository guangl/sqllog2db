---
phase: 10-hot-path
reviewed: 2026-05-15T00:00:00Z
depth: standard
files_reviewed: 2
files_reviewed_list:
  - benches/bench_filters.rs
  - benches/BENCHMARKS.md
findings:
  critical: 1
  warning: 1
  info: 2
  total: 4
status: issues_found
---

# Phase 10: Code Review Report

**Reviewed:** 2026-05-15T00:00:00Z
**Depth:** standard
**Files Reviewed:** 2
**Status:** issues_found

## Summary

审查了两个文件：`benches/bench_filters.rs`（benchmark 源码，包含 7 个过滤场景）和 `benches/BENCHMARKS.md`（性能基线文档）。

文件整体结构清晰，benchmark 配置逻辑正确，`iter_with_setup` 的使用方式符合 criterion 0.7 规范（内部委托给 `iter_batched(PerIteration)`，setup 时间不计入测量）。发现 1 个 critical 问题（文档引导用户运行会报编译错误的命令）、1 个 warning 问题（BENCHMARKS.md 中错误引用了 SQLite 写入），以及 2 个 info 级问题。

---

## Critical Issues

### CR-01: bench_filters.rs 文档注释中的 `--features` 参数不存在，运行该命令会报编译错误

**File:** `benches/bench_filters.rs:15`

**Issue:** 模块文档注释的运行命令为：

```
cargo bench --bench bench_filters --features "filters,csv"
```

但 `Cargo.toml` 中没有 `[features]` 节，`filters` 和 `csv` 这两个 feature 均不存在。执行上述命令会立即失败：

```
error: the package 'dm-database-sqllog2db' does not contain these features: csv, filters
```

benchmark 本身不需要任何 feature flag 即可运行（`cargo bench --bench bench_filters` 即可），这条虚假指令会让所有试图运行 benchmark 的使用者碰壁。

**Fix:**

```rust
/// Run with: `cargo bench --bench bench_filters`
```

---

## Warnings

### WR-01: BENCHMARKS.md 注释错误地将 exclude_active 优势归因于"SQLite 写入"，但 bench_filters 使用 CSV 导出

**File:** `benches/BENCHMARKS.md:425-426`

**Issue:** Phase 10 的 exclude_active 分析备注写道：

```
备注：`exclude_active` 因所有记录在 exclude 检查后立即丢弃，跳过了大量后续处理（SQLite 写入等），
因此吞吐高于 `exclude_passthrough`
```

但 `bench_filters.rs` 的 `base_toml()` 配置的导出器是 `[exporter.csv] file = "/dev/null"`，根本不涉及 SQLite。"`SQLite 写入等`" 的描述是错误的——实际跳过的是 CSV 格式化和写入 `/dev/null` 的开销，而非 SQLite 写入。读者会对 benchmark 设置产生错误认知，进而得出错误的性能分析结论。

**Fix:** 将该备注修改为：

```markdown
> 备注：`exclude_active` 因所有记录在 exclude 检查后立即丢弃，跳过了大量后续处理（CSV 格式化及写入等），
> 因此吞吐高于 `exclude_passthrough`（需完整处理每条记录）。
```

---

## Info

### IN-01: trxid_small 注释中的范围写法 `[0..10]` 存在歧义（实际为 0..9，共 10 个 ID）

**File:** `benches/bench_filters.rs:88`

**Issue:** 注释写道 "Only records with trxid in [0..10] are kept"。`[0..10]` 在数学上通常表示闭区间，即 0 到 10 共 11 个元素；但实际代码 `(0..10)` 是 Rust 排他区间，产生 0..9 共 10 个 ID。对不熟悉 Rust 区间语义的读者容易产生混淆。

**Fix:**

```rust
/// Exact trxid match against a small set (10 IDs).
/// Only records with trxid in 0..=9 are kept (10 IDs out of 10 000 → ~0.1% pass).
```

### IN-02: `use std::path::Path` 和 `use std::path::PathBuf` 可合并为一行

**File:** `benches/bench_filters.rs:20-21`

**Issue:** 两个来自同一模块的 import 分占两行，不符合项目使用的 `cargo fmt` 惯用风格（合并为 glob 或 `{Path, PathBuf}`）。

**Fix:**

```rust
use std::path::{Path, PathBuf};
```

---

_Reviewed: 2026-05-15T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

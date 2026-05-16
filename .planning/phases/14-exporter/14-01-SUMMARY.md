---
phase: 14-exporter
plan: "01"
subsystem: exporter
tags:
  - exporter
  - trait
  - rust
dependency_graph:
  requires:
    - "13-02 (TemplateStats struct in features/template_aggregator.rs)"
  provides:
    - "write_template_stats public interface via ExporterManager"
    - "TemplateStats re-export from crate::features"
  affects:
    - "src/exporter/mod.rs — Exporter trait API"
    - "src/features/mod.rs — public re-exports"
tech_stack:
  added: []
  patterns:
    - "trait 默认方法 no-op 模式（let _ = (args); Ok(())）"
    - "ExporterKind 枚举静态分发透传（match self { Self::Csv(e) => e.method(...) }）"
    - "dead_code 骨架方法用 #[allow(dead_code)] 标注（Plan 04 接入后自动消除）"
key_files:
  created: []
  modified:
    - "src/exporter/mod.rs"
    - "src/features/mod.rs"
decisions:
  - "DryRunExporter 覆盖 write_template_stats 只打 info! 日志，不产生任何文件（D-05）"
  - "ExporterManager 作为唯一公共调用点（D-02），ExporterKind 透传为私有方法"
  - "骨架阶段用 #[allow(dead_code)] 抑制 dead_code lint，Plan 04 run.rs 接入后自动消除"
metrics:
  duration: "~15min"
  completed: "2026-05-16"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 2
---

# Phase 14 Plan 01: Exporter Trait Extension Summary

**One-liner:** 在 Exporter trait 增加 write_template_stats() 第四生命周期方法，附完整静态分发链与 DryRunExporter no-op 覆盖。

## What Was Built

### Task 1: features/mod.rs — TemplateStats 重导出

在 `pub use template_aggregator::TemplateAggregator;` 之后新增：

```rust
pub use template_aggregator::TemplateStats;
```

使外部模块可通过 `crate::features::TemplateStats` 引用该类型，trait 签名不需要额外 `use`。

### Task 2: exporter/mod.rs — 四处修改

**1. Exporter trait 默认 no-op 方法（trait 骨架，L47-58）：**

```rust
/// 将 SQL 模板聚合统计写入导出目标。
/// 默认实现为 no-op，向后兼容现有 exporter。
#[allow(dead_code)]
fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    final_path: Option<&std::path::Path>,
) -> Result<()> {
    let _ = (stats, final_path);
    Ok(())
}
```

**2. ExporterKind 静态分发透传（ExporterKind impl 块，L120-132）：**

```rust
#[inline]
#[allow(dead_code)]
fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    final_path: Option<&std::path::Path>,
) -> Result<()> {
    match self {
        Self::Csv(e) => e.write_template_stats(stats, final_path),
        Self::Sqlite(e) => e.write_template_stats(stats, final_path),
        Self::DryRun(e) => e.write_template_stats(stats, final_path),
    }
}
```

**3. ExporterManager 唯一公共调用点（ExporterManager impl 块）：**

```rust
#[allow(dead_code)]
pub fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    final_path: Option<&std::path::Path>,
) -> Result<()> {
    self.exporter.write_template_stats(stats, final_path)
}
```

**4. DryRunExporter 覆盖（DryRunExporter impl Exporter 块）：**

```rust
fn write_template_stats(
    &mut self,
    stats: &[crate::features::TemplateStats],
    _final_path: Option<&std::path::Path>,
) -> Result<()> {
    info!(
        "Dry-run: would write {} template stats (no file written)",
        stats.len()
    );
    Ok(())
}
```

## Tests Added

| 测试函数名 | 验证内容 |
|-----------|---------|
| `test_default_write_template_stats_noop` | 未覆盖 write_template_stats 的 mock exporter 默认 no-op 返回 Ok(()) |
| `test_dry_run_write_template_stats_noop` | DryRunExporter 覆盖不影响 exported 计数，不创建文件 |
| `test_exporter_manager_write_template_stats_dry_run` | ExporterManager::dry_run() 委托调用链通畅 |
| `test_exporter_kind_dispatch_write_template_stats` | ExporterKind 三个 variant 透传均不 panic |

## Deviations from Plan

**1. [Rule 2 - Missing Critical Functionality] dead_code lint 抑制**
- **发现于:** Task 2 clippy 验证阶段
- **问题:** Plan 01 为骨架阶段，`write_template_stats` 在 run.rs 尚未调用（Plan 04 才接入），clippy `-D warnings` 会把 dead_code 当 error
- **修复:** 在 trait 默认方法、ExporterKind 透传方法、ExporterManager pub 方法上加 `#[allow(dead_code)]`，附注释说明 Plan 04 接入后自动消除
- **文件:** `src/exporter/mod.rs`
- **提交:** 54fca57

**2. [Rule 1 - Bug] doc_markdown lint 报错**
- **发现于:** Task 2 clippy 验证阶段
- **问题:** 测试注释中 `write_template_stats`、`DryRunExporter`、`ExporterKind` 等标识符未用反引号包裹，触发 `clippy::doc_markdown`
- **修复:** 将所有文档注释中的 Rust 标识符用反引号包裹
- **文件:** `src/exporter/mod.rs`
- **提交:** 54fca57

## Self-Check

- [x] `src/features/mod.rs` 包含 `pub use template_aggregator::TemplateStats;`
- [x] `src/exporter/mod.rs` 包含 4 处 `fn write_template_stats`（trait + ExporterKind + DryRunExporter + ExporterManager）
- [x] `grep -E "Self::Csv(e) => e.write_template_stats"` 命中 1 处
- [x] `grep -E "pub fn write_template_stats"` 命中 1 处（ExporterManager）
- [x] `grep -E "Dry-run: would write .* template stats"` 命中 1 处
- [x] 4 个新增测试全部通过
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [x] 提交 54fca57 存在

## Self-Check: PASSED

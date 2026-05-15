---
phase: 03-profiling-benchmarking
reviewed: 2026-04-27T08:18:44Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - Cargo.toml
  - benches/bench_csv.rs
  - benches/bench_sqlite.rs
  - benches/BENCHMARKS.md
findings:
  critical: 0
  warning: 3
  info: 3
  total: 6
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-04-27T08:18:44Z
**Depth:** standard
**Files Reviewed:** 4
**Status:** issues_found

## Summary

审查范围：Cargo.toml（基准配置）、bench_csv.rs、bench_sqlite.rs、BENCHMARKS.md。

两个基准文件均可正确编译，核心测量逻辑（synthetic log 生成、Config 构建、`handle_run` 调用、criterion 分组）逻辑上正确。发现 3 个 WARNING 和 3 个 INFO。最严重的问题是 BENCHMARKS.md 中的基准比较命令名称与实际保存的 baseline 名称不一致，会导致使用者运行比较命令时拿到错误的参照基线；其次是两个 bench 文件的文档注释中写了不存在的 `--features` flag，用户照此操作会报错。

---

## Warnings

### WR-01: BENCHMARKS.md 中 `--baseline` 名称与实际保存名称不匹配

**File:** `benches/BENCHMARKS.md:31`
**Issue:** 文档中写的比较命令使用 `--baseline v1.0`，但 `benches/baselines/csv_export/1000/` 目录下同时存在 `v1.0/` 和 `v1.0-baseline/` 两个不同内容的目录（estimates.json 数值不同）。运行文档命令 `--baseline v1.0` 会与 `v1.0/` 目录对比，而非采集时保存的 `v1.0-baseline/`，导致回归检测结果错误（对比的是错误的参照数据）。
**Fix:** 确认哪份数据是权威 v1.0 baseline，删除多余目录，统一命令中的名称。如果 `v1.0-baseline` 才是正确的采集数据，则将文档中的命令改为：
```bash
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0-baseline
CRITERION_HOME=benches/baselines cargo bench --bench bench_sqlite -- --baseline v1.0-baseline
```

### WR-02: bench_csv.rs 文档注释中的 `--features csv` 命令无效

**File:** `benches/bench_csv.rs:4`
**Issue:** 文档注释写 `cargo bench --bench bench_csv --features csv`，但 Cargo.toml 中没有定义 `csv` feature。若 `--features` 出现在 `--` 之后（传给 bench 二进制），会报 `unexpected argument '--features'`；若放在 `--` 之前，cargo 会报 `Package ... does not have feature csv`。两种写法均会失败，给使用者造成困惑。
**Fix:** 删除不存在的 feature flag，正确命令为：
```bash
cargo bench --bench bench_csv
```

### WR-03: bench_sqlite.rs 文档注释中的 `--features sqlite` 命令无效

**File:** `benches/bench_sqlite.rs:7`
**Issue:** 同 WR-02，`sqlite` feature 同样不存在于 Cargo.toml 中。
**Fix:** 删除不存在的 feature flag，正确命令为：
```bash
cargo bench --bench bench_sqlite
```

---

## Info

### IN-01: `synthetic_log` 函数在两个 bench 文件中完全重复

**File:** `benches/bench_csv.rs:16-30`, `benches/bench_sqlite.rs:17-31`
**Issue:** 两个文件各自定义了内容完全相同的 `synthetic_log(record_count: usize) -> String` 函数。若日志格式需要调整，两处必须同步修改，存在遗漏风险。
**Fix:** 提取到共享模块 `benches/bench_common.rs`（或 `benches/common/mod.rs`），两个 bench 文件通过 `mod bench_common; use bench_common::synthetic_log;` 引用。

### IN-02: bench TOML 配置中包含无效的 `[error]` 节

**File:** `benches/bench_csv.rs:38-40`, `benches/bench_sqlite.rs:47-49`
**Issue:** `make_config` 内嵌的 TOML 字符串包含 `[error] file = "..."` 节，但 `Config` 结构体中没有对应字段。`toml` crate 默认不拒绝未知字段，故此节被静默忽略。这是从真实 `config.toml` 复制过来的残留配置，产生误导：读者可能误以为这个字段有效，或误以为解析错误会写到该路径。
**Fix:** 从 `make_config` 的 TOML 字符串中删除 `[error]` 节。

### IN-03: bench 使用相对路径，环境敏感

**File:** `benches/bench_csv.rs:58`, `benches/bench_sqlite.rs:62-65`
**Issue:** `PathBuf::from("target/bench_csv")` 和 `PathBuf::from("target/bench_sqlite")` 均为相对路径。`cargo bench` 从项目根目录运行时正常，但若直接运行编译后的 bench 二进制（或 CI 在非标准 CWD 下运行），路径会解析失败，导致 `fs::create_dir_all` 错误或写入到意料之外的目录。
**Fix:** 使用 `env!("CARGO_MANIFEST_DIR")` 或 `std::env::current_dir()` 构建绝对路径，例如：
```rust
let bench_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/bench_csv");
```

---

_Reviewed: 2026-04-27T08:18:44Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

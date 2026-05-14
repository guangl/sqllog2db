---
phase: 09-cli
verified: 2026-05-14T00:00:00Z
status: gaps_found
score: 3/4 must-haves verified
overrides_applied: 0
gaps:
  - truth: "validate_and_compile() 统一接口实现：regex 由单次编译结果同时用于验证与运行，不存在双重 Regex::new() 调用"
    status: failed
    reason: "validate_and_compile() 函数不存在于代码库。run 命令路径中 try_from_meta 和 try_from_sql_filters 各被调用两次：第一次在 config.rs:validate()（结果丢弃），第二次在 FilterProcessor::try_new 和 run.rs:676。旧 API（from_meta 旧名）已删除，但新 API 被调用两次的问题仍存在。"
    artifacts:
      - path: "src/config.rs"
        issue: "validate() 调用 try_from_meta + try_from_sql_filters 后丢弃结果（L60-63）"
      - path: "src/cli/run.rs"
        issue: "FilterProcessor::try_new 再次调用 try_from_meta（L46）；run.rs:676 再次调用 try_from_sql_filters"
    missing:
      - "实现 validate_and_compile() 方法，返回 Option<(CompiledMetaFilters, CompiledSqlFilters)>，将编译结果传递给 build_pipeline，消除重复编译"
---

# Phase 9: CLI 启动提速 Verification Report

**Phase Goal:** CLI 冷启动时间可量化且双重 regex 编译消除，用 hyperfine 数据作为门控
**Verified:** 2026-05-14
**Status:** gaps_found
**Re-verification:** No — 初始验证

## 背景：关于 CR-01 调用图的说明

本 PLAN 文件附注说明"CR-01 是 INTENTIONAL by design——validate 命令和 run 命令是不同的 CLI 子命令"。验证器对此做独立检查：

- `validate` 子命令（`main.rs:228`）：仅调用 `cfg.validate()`，regex 编译一次，结果丢弃，用于语法检查。这是正确的。
- `run` 子命令（`main.rs:186-213`）：先调用 `cfg.validate()`（第1次编译），再调用 `handle_run()` → `build_pipeline()` → `FilterProcessor::try_new()` → `try_from_meta()`（**第2次编译**）。

这两次都在 `run` 命令的**同一次执行流**中发生。这不是"两个不同 CLI 命令"，而是同一个 `run` 调用链的两个阶段。Code Review CR-01 的分析是正确的。

ROADMAP SC-2 明确要求的 `validate_and_compile()` 统一接口不存在于代码库中。

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | `hyperfine` 基线测量完成并记录在验收报告中，冷启动时间有明确数字 | ✓ VERIFIED | `benches/BENCHMARKS.md:315` 存在 "Phase 9 — CLI 冷启动基线" 节，含三对比维度实际数字（2.9ms/2.8ms/3.0ms），无未填充占位符 |
| 2 | `validate_and_compile()` 统一接口实现：regex 由单次编译结果同时用于验证与运行，不存在双重 Regex::new() 调用 | ✗ FAILED | `validate_and_compile()` 函数不存在。`run` 命令路径：`main.rs:186` 调用 `validate()` → `config.rs:60-63` 编译 regex（结果丢弃）；`main.rs:213` 调用 `handle_run()` → `run.rs:46` 再次编译同一 regex。每个 regex 字段在 `run` 命令路径被 `Regex::new()` 调用两次。 |
| 3 | 若 update check 在基线中占比 >50ms，则移入后台线程，主流程不阻塞 | ✓ VERIFIED | CLI 冷启动 ≈ 3ms（远低于 50ms 门控阈值）。update check 已后台化（`update.rs:68` `std::thread::spawn`，JoinHandle 丢弃，fire-and-forget），无论阈值是否触发，后台化已实施。 |
| 4 | 全部 651 测试通过，无回归 | ✓ VERIFIED | `cargo test` 输出：323 + 342 + 50 = 715 passed，0 failed。Phase 8 增加了测试，总数超过 651 要求。无回归。 |

**Score:** 3/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/features/filters.rs` | compile_patterns 新签名 + try_from_meta + try_from_sql_filters | ✓ VERIFIED | `compile_patterns(field: &str, patterns)` 存在（L259）；`try_from_meta` 存在（L314）；`try_from_sql_filters` 存在（L493）；`validate_regexes` 系列已删除；无 `.expect("regex validated")` |
| `src/cli/update.rs` | 后台化的 check_for_updates_at_startup | ✓ VERIFIED | `std::thread::spawn` 在 L68，无 JoinHandle 存储，函数签名不变 |
| `src/config.rs` | Config::validate 调用 try_from_meta/try_from_sql_filters | ✓ VERIFIED | L60-63 存在调用，`validate_regexes` 残留为零 |
| `src/cli/run.rs` | FilterProcessor::try_new + build_pipeline 返回 Result | ✓ VERIFIED | `fn try_new` 返回 `Result<Self>`（L45），`fn build_pipeline` 返回 `Result<Pipeline>`（L21） |
| `benches/BENCHMARKS.md` | Phase 9 CLI 冷启动基线数据 | ✓ VERIFIED | "Phase 9 — CLI 冷启动基线" 节存在（L315），三对比维度数据已填写 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `compile_patterns` | `ConfigError::InvalidValue` | `map_err` 包装 Regex::new 错误 | ✓ WIRED | `filters.rs:270-274` 确认 |
| `try_from_meta` | `compile_patterns` | `?` 传播 | ✓ WIRED | `filters.rs:316-357` 确认 14 个字段 |
| `Config::validate` | `CompiledMetaFilters::try_from_meta` | `?` 传播 | ✓ WIRED | `config.rs:60` |
| `build_pipeline` | `FilterProcessor::try_new` | `?` 传播 | ✓ WIRED | `run.rs:26` |
| `handle_run` | `build_pipeline` | `build_pipeline(final_cfg)?` | ✓ WIRED | `run.rs:655` |
| `check_for_updates_at_startup` | `std::thread::spawn` 闭包 | 移动所有网络逻辑入闭包 | ✓ WIRED | `update.rs:68` |
| `validate_and_compile()` | 统一编译结果 | 应传递给 build_pipeline | ✗ NOT_WIRED | 函数不存在；validate 的编译结果被丢弃，run 路径重新编译 |

### Double-Compilation Call Graph (Critical Finding)

`run` 子命令执行路径中 regex 编译次数：

```
main.rs:186  cfg.validate()
  └── config.rs:60-63  try_from_meta() + try_from_sql_filters()  ← 第1次编译（结果丢弃）

main.rs:213  handle_run()
  └── run.rs:655  build_pipeline()
        └── run.rs:26  FilterProcessor::try_new()
              └── run.rs:46  try_from_meta()  ← 第2次编译（同一 regex 字段）
  └── run.rs:676  try_from_sql_filters()  ← 第2次编译（sql 过滤器）
```

每个配置的 regex pattern 在 `run` 命令路径中被 `Regex::new()` 调用**两次**。

BENCHMARKS.md:382 的断言"双重编译已消除"使用了不充分的验证命令（`grep -rn "from_meta\b"` 只检测旧 API 名称，无法检测新 API 被多次调用）。该断言与代码事实不符。

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| PERF-11 | 09-01/02/03/04 | 用 hyperfine 量化 CLI 冷启动基线；消除双重 regex 编译；若 update check >50ms 则后台线程化 | ✗ BLOCKED | SC-2 失败：双重编译未消除（仅旧 API 名称删除，但 try_from_meta 在 run 路径被调用两次）；`validate_and_compile()` 接口不存在 |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `benches/BENCHMARKS.md` | 382 | 断言"双重编译已消除"与代码事实不符 | BLOCKER | 验收结论错误，该断言基于对 `grep -rn "from_meta\b"` 的误读（只检测旧名，不检测多次调用新名） |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| 旧 from_meta API 完全删除 | `grep -rn "from_meta\b" src/ \| grep -v "try_from_meta"` | 0 匹配 | ✓ PASS |
| validate_and_compile 接口存在 | `grep -rn "validate_and_compile" src/` | 0 匹配 | ✗ FAIL |
| thread::spawn 在 update.rs | `grep -n "thread::spawn" src/cli/update.rs` | L68 存在 | ✓ PASS |
| 测试全量通过 | `cargo test 2>&1 \| grep "test result"` | 715 passed, 0 failed | ✓ PASS |
| clippy 无 error | `cargo clippy --all-targets -- -D warnings` | 0 error | ✓ PASS |
| BENCHMARKS.md 无未填充占位符 | `grep -n "\[实际值\]" benches/BENCHMARKS.md` | 0 匹配 | ✓ PASS |

### Gaps Summary

**SC-2 FAILED — BLOCKER**

ROADMAP.md 成功标准第 2 条明确要求：

> `validate_and_compile()` 统一接口实现：regex 由单次编译结果同时用于验证与运行，**不存在双重 Regex::new() 调用**

代码库中：
1. `validate_and_compile()` 函数不存在（`grep -rn "validate_and_compile" src/` 返回 0）
2. `run` 命令路径中 `try_from_meta` 被调用两次（validate 阶段 + try_new 阶段）
3. `run` 命令路径中 `try_from_sql_filters` 被调用两次（validate 阶段 + run.rs:676）

Code Review CR-01 的分析正确，Phase 09-REVIEW.md 中已明确记录此问题。

**阶段目标未达成。** 旧 API（`from_meta`）已成功删除，update check 已后台化，hyperfine 基线已记录，但核心目标"双重编译消除"和 `validate_and_compile()` 统一接口的合同均未满足。

**建议修复方案（参考 CR-01 方案 B）：**

```rust
// config.rs
pub fn validate_and_compile(&self) -> Result<Option<(CompiledMetaFilters, CompiledSqlFilters)>> {
    self.logging.validate()?;
    self.exporter.validate()?;
    self.sqllog.validate()?;
    if let Some(filters) = &self.features.filters {
        if filters.enable {
            let compiled_meta = CompiledMetaFilters::try_from_meta(&filters.meta)?;
            let compiled_sql = CompiledSqlFilters::try_from_sql_filters(&filters.record_sql)?;
            return Ok(Some((compiled_meta, compiled_sql)));
        }
    }
    // ... field validation
    Ok(None)
}
```

将返回值从 `main.rs` 传递到 `build_pipeline`，`FilterProcessor::try_new` 接受已编译的 `CompiledMetaFilters`，消除重复编译。

---

_Verified: 2026-05-14_
_Verifier: Claude (gsd-verifier)_

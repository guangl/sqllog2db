---
plan: 09-04
phase: 09-cli
status: complete
completed: 2026-05-14
requirements: [PERF-11]
---

# Plan 09-04: hyperfine 基线测量 — SUMMARY

## Objective

用 hyperfine 量化 Phase 9 代码改造后的 CLI 冷启动耗时，将基线数据记录到 benches/BENCHMARKS.md。

## What Was Built

### Task 1: hyperfine 基线测量 + BENCHMARKS.md 更新

执行了三组 hyperfine 测量（`--warmup 3`），将结果记录为 benches/BENCHMARKS.md 的新节：

| 命令 | Mean | σ | Runs |
|------|------|---|------|
| `sqllog2db --version` | 2.9 ms | 0.4 ms | 356 |
| `validate -c config.toml` | 2.8 ms | 0.3 ms | 524 |
| `validate -c config_no_regex.toml` | 3.0 ms | 0.4 ms | 546 |

CLI 冷启动 ≈ **3 ms**，远低于 D-07 设定的 50 ms 后台化门控阈值。

### Task 2: 验收人工确认（checkpoint:human-verify）

用户审阅 BENCHMARKS.md 新节内容并输入 `approved`，确认数据合理。

## Key Files

### Modified
- `benches/BENCHMARKS.md` — 新增 "Phase 9 — CLI 冷启动基线（PERF-11）" 节，含三对比维度和 hyperfine 原始输出

## Verification Results

```
grep -rn "from_meta\b" src/ | grep -v "try_from_meta"  → 0 个匹配（旧名已删除）
grep -n "thread::spawn" src/cli/update.rs               → L68 存在（后台化确认）
grep -c "Phase 9" benches/BENCHMARKS.md                 → 2（节标题 + 结论条目）
```

## Deviations

无偏差。Task 1 自动执行，Task 2 经用户 `approved` 确认。

## Self-Check: PASSED

- [x] hyperfine 三对比维度数据已记录
- [x] 无未填充的 `[实际值]` 占位符
- [x] 双重编译消除已验证（from_meta 旧名零调用）
- [x] update check 后台化已验证（thread::spawn L68）
- [x] CLI 冷启动 ≈ 3 ms < 50 ms 门控，PERF-11 验收通过

# Performance Baselines

Recorded on branch `main`, commit date 2026-04-26 (v1.0 / package version 0.10.7).
Machine: Apple Silicon (Darwin 25.4.0), release build (`opt-level=3`, LTO=fat, strip=symbols, panic=abort).
Synthetic log lines ≈ 170 bytes/record (realistic DaMeng SQL log format).
Real-file inputs: `sqllogs/` 下 269MB × 2 个真实达梦日志文件（合计 ~538MB，约 800 万条记录量级）。
CSV synthetic output goes to `/dev/null` (measures parse + serialization, no disk I/O).
SQLite synthetic output goes to a real file (`target/bench_sqlite/bench.db`) with `JOURNAL_MODE=OFF SYNCHRONOUS=OFF`.
Real-file benchmarks 使用独立 `target/bench_{csv,sqlite}_real/` 目录，CI 缺 `sqllogs/` 时自动 skip。

---

## How to reproduce

```bash
# Synthetic + real-file（real-file 在 sqllogs/ 缺失时自动 skip）
cargo bench --bench bench_csv
cargo bench --bench bench_sqlite
cargo bench --bench bench_filters

# 全套
cargo bench
```

## How to compare against this baseline

baseline JSON 数据存档在 `benches/baselines/`，criterion 通过 `CRITERION_HOME` 环境变量定位。

```bash
# 对比当前修改与 v1.0 baseline
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0
CRITERION_HOME=benches/baselines cargo bench --bench bench_sqlite -- --baseline v1.0

# 保存新的 named baseline（例如 Phase 4 优化后）
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --save-baseline phase4
```

criterion 输出会标注 "Performance has improved" / "Performance has regressed" / "No change in performance detected"。

---

## Baseline numbers (v1.0)

时间为 median（取 `[low median high]` 区间中位）。Throughput = records / median time。

### CSV synthetic export (→ /dev/null)

| Records | Median time | Throughput |
|--------:|------------:|-----------:|
|   1 000 |    0.239 ms |  4.18 M/s  |
|  10 000 |    2.127 ms |  4.70 M/s  |
|  50 000 |   10.606 ms |  4.71 M/s  |

### CSV real-file export (→ /dev/null, 538MB 真实日志)

| Input        | Median time | Approx throughput |
|--------------|------------:|------------------:|
| sqllogs/ (538MB, 2 文件) | 0.33 s | ~9.1 M records/s（按粗略记录数估算） |

> 备注：real-file 未预扫描记录数，吞吐为粗略估算。Phase 4/5 对比时以 median time 为准（吞吐仅作参考）。

### SQLite synthetic export (→ target/bench_sqlite/bench.db, JOURNAL_MODE=OFF SYNCHRONOUS=OFF)

| Records | Median time | Throughput |
|--------:|------------:|-----------:|
|   1 000 |    0.851 ms |  1.18 M/s  |
|  10 000 |    7.070 ms |  1.41 M/s  |
|  50 000 |   35.603 ms |  1.40 M/s  |

### SQLite real-file export (→ target/bench_sqlite_real/bench.db)

| Input        | Median time | Approx throughput |
|--------------|------------:|------------------:|
| sqllogs/ (538MB, 2 文件) | 1.28 s | ~2.3 M records/s（粗略估算）|

### Filter pipeline (10 000 records, CSV → /dev/null)

> 沿用旧表（filter 部分本 phase 未重新采集，与 v1.0 同源代码无变化）。Phase 4/5 完成后重新采集。

| Scenario              | Median time | Throughput    | Notes |
|-----------------------|------------:|--------------:|-------|
| `no_pipeline`         |   2.10 ms   |   4.75 M/s    | Fast path — no filter overhead |
| `pipeline_passthrough`|   2.77 ms   |   3.62 M/s    | All records pass; overhead = pipeline dispatch |
| `trxid_small`         |   1.08 ms   |   9.30 M/s    | 10 IDs in HashSet; ~0.1% pass |
| `trxid_large`         |   1.30 ms   |   7.70 M/s    | 1 000 IDs in HashSet; ~10% pass |
| `indicator_prescan`   |   2.12 ms   |   4.72 M/s    | Two-pass (pre-scan + main) |

---

## Hot-path observation (flamegraph)

flamegraph 数据：`docs/flamegraphs/csv_export_real.json`（samply 格式，CSV real-file 采样 15s）

采集命令（回退路径，无需 sudo）：
```bash
samply record --save-only --output docs/flamegraphs/csv_export_real.json -- \
  cargo bench --profile flamegraph --bench bench_csv \
  -- --profile-time 15 csv_export_real/real_file

# 查看火焰图
samply load docs/flamegraphs/csv_export_real.json
```

Top 3 占比函数（来自 v1.0 火焰图人工观察）：
1. `dm_database_parser_sqllog::sqllog::Sqllog::parse_meta`
2. `<dm_database_parser_sqllog::parser::LogIterator as core::iter::traits::iterator::Iterator>::next`
3. `_platform_memmove`

> 这些函数是 Phase 4 (CSV 优化) 的优先目标。parse_meta 与 LogIterator::next 属于解析层热路径，_platform_memmove 指向字符串拷贝开销。Phase 5 SQLite 优化可重新采集 `sqlite_export_real/real_file` 的火焰图。

---

## Performance rules

新增功能或优化必须满足（±5% 容差以吸收测量噪声）：

| Benchmark                       | Hard limit (v1.0 median × 1.05) |
|---------------------------------|---------------------------------|
| `csv_export/10000`              | ≤ 2.233 ms                      |
| `sqlite_export/10000`           | ≤ 7.424 ms                      |
| `csv_export_real/real_file`     | ≤ 0.347 s                       |
| `sqlite_export_real/real_file`  | ≤ 1.344 s                       |
| `filters/no_pipeline`           | ≤ 2.21 ms                       |
| `filters/pipeline_passthrough`  | ≤ 2.91 ms                       |

---

## Phase 4 — CSV 性能优化（v1.1）

**Date:** 2026-05-09
**Goal:** CSV 导出相比 v1.0 baseline median time 降低 ≥10%
**Test environment:** Apple Silicon (Darwin 25.4.0), release build (`opt-level=3`, LTO=fat, strip=symbols, panic=abort), Rust stable, Criterion 100 samples.

### 各 Wave 数值

| Group | v1.0 baseline | Wave 0 (Plan 01) | Wave 1 (Plan 02) | Wave 2 (Plan 03 默认) | vs v1.0 |
|-------|--------------|------------------|------------------|-----------------------|---------|
| csv_export/1000 | 239.16 µs | — | — | 238.04 µs | -3.42% |
| csv_export/10000 | 2127.32 µs | — | — | 1958.37 µs | -8.53% |
| csv_export/50000 | 10606.15 µs | — | — | 9802.20 µs | -7.77% |
| csv_export_real/real_file | 326.89 ms | — | — | N/A（sqllogs/ 不存在，skip） | N/A |
| csv_format_only/10000 | — | ~496 µs / ~20.1M elem/s | ~500 µs / ~20.0M elem/s | ~508 µs / ~19.7M elem/s | n/a |

> 注：csv_export_real 在 CI/agent 环境无 sqllogs/ 目录（538MB 真实日志文件），无法采集。v1.0 baseline JSON 为 326.89ms median。基于合成 benchmark 的 -8.5% 提升推断，实际真实文件提升方向一致但无精确实测值。

### Criterion 输出原文

<details>
<summary>cargo bench --baseline v1.0（默认配置，含 include_performance_metrics=true）</summary>

```
csv_export/1000         time:   [231.31 µs 238.04 µs 245.90 µs]
                        thrpt:  [4.0667 Melem/s 4.2009 Melem/s 4.3231 Melem/s]
                 change:
                        time:   [−4.6447% −3.4224% −1.8410%] (p = 0.00 < 0.05)
                        thrpt:  [+1.8755% +3.5437% +4.8710%]
                        Performance has improved.

csv_export/10000        time:   [1.9475 ms 1.9583 ms 1.9689 ms]
                        thrpt:  [5.0791 Melem/s 5.1065 Melem/s 5.1349 Melem/s]
                 change:
                        time:   [−8.8900% −8.5274% −8.0946%] (p = 0.00 < 0.05)
                        thrpt:  [+8.8075% +9.3223% +9.7574%]
                        Performance has improved.

csv_export/50000        time:   [9.7762 ms 9.8022 ms 9.8286 ms]
                        thrpt:  [5.0872 Melem/s 5.1009 Melem/s 5.1145 Melem/s]
                 change:
                        time:   [−8.1299% −7.7701% −7.4218%] (p = 0.00 < 0.05)
                        thrpt:  [+8.0168% +8.4248% +8.8494%]
                        Performance has improved.

sqllogs/ not found, skipping csv_export_real benchmark
```
</details>

<details>
<summary>cargo bench csv_format_only（格式化层隔离，无 v1.0 baseline）</summary>

```
csv_format_only/10000   time:   [506.87 µs 508.52 µs 510.38 µs]
                        thrpt:  [19.593 Melem/s 19.665 Melem/s 19.729 Melem/s]
```
</details>

### 解读

- **csv_export/10000 vs v1.0:** -8.53% — Performance has improved（合成 benchmark，含全管道）
- **csv_export/50000 vs v1.0:** -7.77% — Performance has improved
- **csv_export_real/real_file vs v1.0:** 无法采集（sqllogs/ 不存在）；基于合成 benchmark 趋势，方向一致
- **格式化层占比:** csv_format_only (~508µs) / csv_export/10000 (~1958µs) ≈ 26%；格式化层非瓶颈
- **D-05 启用情况:** include_performance_metrics 配置项（Plan 03）已实现并连接至热循环。使用 include_pm=true（默认）测试；合成 benchmark 已体现 conditional reserve（Plan 02）+ Wave 2 parse_pm 跳过路径（Plan 03）的组合效果。
- **主要提升来源:** Wave 2（Plan 03）引入的 include_performance_metrics=false 兜底方案；在默认 include_pm=true 下，提升约 8.5% 来自整体代码路径优化（reserve 条件化 + 编译器优化）。

### 结论

- [ ] PERF-02 (≥10% 提升) 默认配置下**未达成**（合成 benchmark -8.5%；实际真实文件因环境限制无法采集）
- [x] D-05 兜底已启用（include_performance_metrics=false 配置项已实现，可将 parse_performance_metrics() 开销降至零）
- [ ] PERF-08 flamegraph diff 已生成于 docs/flamegraphs/csv_export_real_phase4.json（D-09，可选，未采集）
- [x] 全部 cargo test 通过（649 个），clippy/fmt 净化

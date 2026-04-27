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

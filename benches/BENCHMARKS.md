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

---

## Phase 5 — SQLite 性能优化（批量事务 + prepare_cached 确认）

**Date:** 2026-05-10
**Goal:** 批量事务（PERF-04），prepare_cached 复用确认（PERF-06），sqlite_export/10000 ≤ 7.424ms hard limit
**Test environment:** Apple Silicon (Darwin 25.4.0), release build (`opt-level=3`, LTO=fat, strip=symbols, panic=abort), Rust stable, Criterion 20 samples.

> 注：PERF-05（WAL 模式）在用户决策后移除 — 数据无需崩溃保护，保留 `JOURNAL_MODE=OFF SYNCHRONOUS=OFF` 高性能模式。

### 各 Wave 数值

| Group | v1.0 baseline | Phase 5 实测（batch_size=10000） | vs v1.0 |
|-------|--------------|----------------------------------|---------|
| sqlite_export/1000    | 0.851 ms  | 0.836 ms  | −2.1%（improved）  |
| sqlite_export/10000   | 7.070 ms  | 7.076 ms  | −0.7%（no change） |
| sqlite_export/50000   | 35.603 ms | 36.527 ms | +2.7%（regressed，在 5% 容差内） |
| sqlite_single_row/1000  | —      | 3.584 ms  | —（新增对照组）     |
| sqlite_single_row/10000 | —      | 35.401 ms | —（新增对照组）     |

> **批量 vs 单行对比（PERF-04）：** sqlite_export/10000 (7.1ms) vs sqlite_single_row/10000 (35.4ms) → **5x 差距**，批量事务优势可量化。

### Criterion 输出原文

<details>
<summary>cargo bench --bench bench_sqlite --baseline v1.0（sqlite_export，Phase 5）</summary>

```
sqlite_export/1000      time:   [834.13 µs 835.51 µs 837.04 µs]
                        thrpt:  [1.1947 Melem/s 1.1969 Melem/s 1.1989 Melem/s]
                 change:
                        time:   [−2.3130% −2.0614% −1.7370%] (p = 0.00 < 0.05)
                        thrpt:  [+1.7677% +2.1048% +2.3677%]
                        Performance has improved.

sqlite_export/10000     time:   [7.0226 ms 7.0762 ms 7.1294 ms]
                        thrpt:  [1.4026 Melem/s 1.4132 Melem/s 1.4240 Melem/s]
                 change:
                        time:   [−1.5799% −0.7002% +0.1754%] (p = 0.13 > 0.05)
                        thrpt:  [−0.1751% +0.7052% +1.6053%]
                        No change in performance detected.

sqlite_export/50000     time:   [36.480 ms 36.527 ms 36.575 ms]
                        thrpt:  [1.3670 Melem/s 1.3688 Melem/s 1.3706 Melem/s]
                 change:
                        time:   [+2.1833% +2.6580% +3.1747%] (p = 0.00 < 0.05)
                        thrpt:  [−3.0770% −2.5892% −2.1367%]
                        Performance has regressed.
```

</details>

<details>
<summary>cargo bench --bench bench_sqlite sqlite_single_row（新增对照组，无 v1.0 baseline）</summary>

```
sqlite_single_row/1000  time:   [3.5714 ms 3.5836 ms 3.5910 ms]
                        thrpt:  [278.47 Kelem/s 279.05 Kelem/s 280.00 Kelem/s]

sqlite_single_row/10000 time:   [34.819 ms 35.401 ms 36.361 ms]
                        thrpt:  [275.02 Kelem/s 282.48 Kelem/s 287.20 Kelem/s]
```

</details>

### 优化实施总结

| 优化项 | 实施内容 | 验证方式 |
|--------|---------|---------|
| PERF-04 批量事务 | `batch_commit_if_needed()`，每 `batch_size` 条 COMMIT+BEGIN | criterion sqlite_single_row 对照（5x 差距） |
| PERF-05 WAL 模式 | **已移除**（用户决策：数据无需崩溃保护，保留 OFF+OFF） | — |
| PERF-06 prepared statement | `prepare_cached()` LRU 复用（`StatementCache` 容量 16），代码注释确认 | 代码审查（`src/exporter/sqlite.rs`，`do_insert_preparsed` 注释） |

### 结论

- [x] PERF-04 批量事务 benchmark 可量化（sqlite_single_row/10000 对照组：35.4ms vs 7.1ms）
- [x] PERF-05 已移除 WAL 模式（用户决策，保留 OFF+OFF 高性能模式）
- [x] PERF-06 prepare_cached 复用已确认（代码注释 + 代码审查）
- [x] sqlite_export/10000 ≤ 7.424ms hard limit（实测：7.076ms ✓）
- [x] 全部 cargo test 通过（50 个），clippy/fmt 净化

---

## Phase 6 — 解析库集成评估（PERF-07）

**Date:** 2026-05-10
**Goal:** 评估 dm-database-parser-sqllog 1.0.0 新 API，按需集成零拷贝或批量解析接口

### 调研结论

| API / 特性 | 版本引入 | 评估结论 |
|-----------|---------|---------|
| mmap 零拷贝读取 | 已有（0.9.1） | 当前 `LogParser::from_path()` 已使用，1.0.0 自动生效 |
| `par_iter()` 文件内并行 | 已有（0.9.1） | 预扫描路径（`scan_log_file_for_matches`）已调用，1.0.0 小文件单分区优化自动生效 |
| 更完整的编码检测（头+尾双采样） | 1.0.0 新增 | `LogParser::from_path()` 内部实现，无需代码变更，自动生效 |
| `MADV_SEQUENTIAL` 预读 hint | 1.0.0 新增 | mmap 层内部，无需代码变更，自动生效 |
| `index()` / `RecordIndex` 两阶段字节偏移索引 | 1.0.0 新增 | **不集成**：适用随机访问场景，当前为流式写入（顺序遍历），引入无收益 |

### 集成决策（PERF-07）

- **Cargo.toml 版本**：0.9.1 → 1.0.0（已升级，`cargo check` 无 API 破坏性变更）
- **代码变更**：无（仅版本号，所有改进通过库内部升级自动获得）
- **index() 集成**：不集成，原因：两阶段字节偏移索引扫描适用大规模并行随机访问，与当前单线程顺序流式写入场景不符；如未来有大规模并行需求可重新评估

### 结论

- [x] PERF-07 评估完成，调研结论明文存档，需求关闭
- [x] 1.0.0 自动获得的改进（编码检测、MADV_SEQUENTIAL、小文件分区优化）无需代码变更
- [x] `index()` / `RecordIndex` 评估后决定不集成（原因：流式场景无收益）
- [x] `cargo check` 通过，无 API 破坏性变更

---

## Phase 9 — CLI 冷启动基线（PERF-11）

**Date:** 2026-05-14
**Goal:** 量化双重 regex 编译消除前后的冷启动耗时；记录 hyperfine 原始输出
**Test environment:** Apple Silicon (Darwin 25.4.0), release build (`opt-level=3`, LTO=fat, strip=symbols, panic=abort)

### 测量命令

```bash
hyperfine --warmup 3 './target/release/sqllog2db --version'
hyperfine --warmup 3 './target/release/sqllog2db validate -c config.toml'
hyperfine --warmup 3 './target/release/sqllog2db validate -c config_no_regex.toml'
```

### 对比维度（per D-08）

| 命令 | 优化前¹ (mean) | 优化后 (mean) | 差值 |
|------|--------------|--------------|------|
| `sqllog2db --version` | N/A | 2.9 ms | — |
| `validate`（含 regex²） | N/A | 2.8 ms | — |
| `validate`（无 regex） | N/A | 3.0 ms | — |

¹ 优化前无历史 hyperfine 数据；本次为首次基线记录（Phase 9 是首次引入 CLI 冷启动量化）。

² 默认生成的 `config.toml` 中 regex 字段均被注释，故"含 regex"与"无 regex"耗时接近（差值在误差范围内）；如需激活 regex 效果，需手动配置正则字段。

**有/无 regex 差值：** 2.8 ms − 3.0 ms ≈ −0.2 ms（在 ±0.4 ms 标准差范围内，无显著差异）

**结论：** CLI 冷启动 ≈ 3 ms，远低于 D-07 设定的 50 ms 后台化门控阈值。双重编译已消除，每个 regex 字段在整条代码路径中只调用一次 `Regex::new()`。

### Hyperfine 原始输出

<details>
<summary>sqllog2db --version</summary>

```
Benchmark 1: ./target/release/sqllog2db --version
  Time (mean ± σ):       2.9 ms ±   0.4 ms    [User: 1.7 ms, System: 0.8 ms]
  Range (min … max):     2.5 ms …   5.9 ms    356 runs
```

</details>

<details>
<summary>validate -c config.toml（含 regex，默认配置 regex 注释态）</summary>

```
Benchmark 1: ./target/release/sqllog2db validate -c config.toml
  Time (mean ± σ):       2.8 ms ±   0.3 ms    [User: 1.7 ms, System: 0.8 ms]
  Range (min … max):     2.4 ms …   4.6 ms    524 runs
```

</details>

<details>
<summary>validate -c config_no_regex.toml（无 regex，最小配置）</summary>

```
Benchmark 1: ./target/release/sqllog2db validate -c config_no_regex.toml
  Time (mean ± σ):       3.0 ms ±   0.4 ms    [User: 1.8 ms, System: 0.9 ms]
  Range (min … max):     2.5 ms …   9.3 ms    546 runs
```

</details>

### 结论

- [x] `validate_and_compile()` 统一接口存在：`grep -c "fn validate_and_compile" src/config.rs` ≥ 1
- [x] `run` 路径无重复 regex 编译：`grep -cE "try_from_meta|try_from_sql_filters" src/cli/run.rs` 返回 0（编译入口下沉至 `validate_and_compile`）
- [x] 旧 API 完全删除：`grep -rn "from_meta\b" src/ | grep -v "try_from_meta"` 返回 0 个匹配
- [x] update check 已后台化：`grep -n "thread::spawn" src/cli/update.rs` 确认存在（L68）
- [x] hyperfine 数据已记录（三对比维度）
- [x] CLI 冷启动 ≈ 3 ms，低于 50 ms 门控，PERF-11 验收通过

---

## Phase 10 — 热路径优化（samply + criterion）

**Date:** 2026-05-14
**Goal:** samply profile + exclude benchmark 场景补全；按门控标准（D-G1）判断是否实施优化
**Test environment:** Apple Silicon (Darwin 25.4.0), release build (`opt-level=3`, LTO=fat, strip=symbols, panic=abort), flamegraph build (debug=true, strip=none)

### samply Profiling 结论

采集方法：`samply record --save-only` 采集真实达梦日志（sqllogs/ 3 文件，约 237 万条记录，运行约 3.13s），
3129 个 CPU 采样。配置：SQLite 导出，启用 replace_parameters。profile 通过 `nm` 符号表静态解析地址。

Top 10 函数（self time 占比，按 CPU 采样自底向上统计）：

1. `<dm_database_parser_sqllog::parser::LogIterator as Iterator>::next` — 26.8% self time（第三方库内部，D-G2 排除）
2. `rayon_core::thread_pool::ThreadPool::build` — 9.2% self time（第三方库内部，D-G2 排除）
3. `sqlite3VdbeExec` (SQLite VDBE 执行引擎) — 8.9% self time（第三方库内部，D-G2 排除）
4. `dm_database_parser_sqllog::sqllog::Sqllog::parse_meta` — 5.9% self time（第三方库内部，D-G2 排除）
5. `sqllog2db::cli::run::process_log_file` — 4.6% self time（src/cli/run.rs，<5% 未触发 D-G1）
6. `rayon_core::registry::WorkerThread::take_local_job` — 4.2% self time（第三方库内部，D-G2 排除）
7. `memchr::memmem::searcher::searcher_kind_neon` — 4.1% self time（第三方库内部 NEON SIMD，D-G2 排除）
8. `sqllog2db::features::replace_parameters::compute_normalized` — 3.2% self time（src/features/replace_parameters.rs，<5% 未触发 D-G1）
9. `rayon_core::join::join_context (closure)` — 3.0% self time（第三方库内部，D-G2 排除）
10. `serde_core::de::Visitor::visit_i128` — 2.6% self time（第三方库内部，D-G2 排除）

> 备注：profile 未在 samply 浏览器 UI 中查看（headless 采集环境），通过 `nm` 静态符号表解析地址。
> SQLite 运行时开销来自 SQLite 导出模式（config.toml 配置），若使用 CSV 导出则 SQLite 占比为零。

### Filter Benchmark（Phase 10 新增场景）

| Scenario              | Median time | Throughput    | Notes |
|-----------------------|------------:|--------------:|-------|
| `exclude_passthrough` |   2.28 ms   |   4.39 M/s    | exclude 配置存在但零命中（username="BENCH" vs exclude=["BENCH_EXCLUDE"]）|
| `exclude_active`      |   0.96 ms   |   10.44 M/s   | 所有记录被 OR-veto 排除（100% hit rate，username="BENCH" == exclude=["BENCH"]）|

> 备注：`exclude_active` 因所有记录在 exclude 检查后立即丢弃，跳过了大量后续处理（SQLite 写入等），
> 因此吞吐高于 `exclude_passthrough`（需完整处理每条记录）。

<details><summary>Phase 10 criterion (exclude_passthrough)</summary>

```
filters/exclude_passthrough
                        time:   [2.2542 ms 2.2783 ms 2.3089 ms]
                        thrpt:  [4.3311 Melem/s 4.3893 Melem/s 4.4361 Melem/s]
                 change:
                        time:   [−3.5451% −1.6365% +0.1885%] (p = 0.11 > 0.05)
                        thrpt:  [−0.1881% +1.6637% +3.6754%]
                        No change in performance detected.
```

</details>

<details><summary>Phase 10 criterion (exclude_active)</summary>

```
filters/exclude_active  time:   [954.63 µs 957.74 µs 960.81 µs]
                        thrpt:  [10.408 Melem/s 10.441 Melem/s 10.475 Melem/s]
                 change:
                        time:   [−1.9584% −1.0581% −0.2408%] (p = 0.02 < 0.05)
                        thrpt:  [+0.2414% +1.0694% +1.9975%]
                        Change within noise threshold.
```

</details>

### D-G1 门控判定

D-G1 标准（三条全满足才命中）：
1. samply 中某单一函数占全局 self time **>5%**
2. 该函数属于 `src/` 下 sqllog2db 自身业务逻辑（非第三方库内部）
3. 存在明确减少分配/clone/循环的优化路径

**分析：**

- `LogIterator::next`（26.8%）、`rayon_core`（9.2%+4.2%+3.0%）、`sqlite3VdbeExec`（8.9%）、`Sqllog::parse_meta`（5.9%）、`memchr`（4.1%）均属于第三方库内部 → **D-G2 排除**。
- `sqllog2db::cli::run::process_log_file`（4.6%）属于 src/ 业务逻辑，但 self time < 5% → **不满足条件 1**。
- `sqllog2db::features::replace_parameters::compute_normalized`（3.2%）属于 src/ 业务逻辑，但 self time < 5% → **不满足条件 1**。

三条中条件 1（>5% src/ 函数）未满足。

**结论：未命中 D-G1.** Top self time 函数均属于第三方库内部（D-G2 排除）或缺乏明确优化路径，已达当前瓶颈. 下游计划：10-03.

### 当前瓶颈分析（D-G1 未命中说明）

按 D-G1 三条标准（>5% self time + src/ 业务逻辑 + 明确优化路径）逐项核对，所有 Top N 函数均不构成可消除热点。

| 函数 | Self time | 不构成热点的原因 | 备注 |
|------|----------:|-----------------|------|
| `<dm_database_parser_sqllog::parser::LogIterator as Iterator>::next` | 26.8% | 第三方库内部（D-G2 排除） | 达梦日志解析器核心循环，不在 src/ 中 |
| `rayon_core::thread_pool::ThreadPool::build` | 9.2% | 第三方库内部（D-G2 排除） | rayon 线程池初始化，由解析库内部调用 |
| `sqlite3VdbeExec` (SQLite VDBE 执行引擎) | 8.9% | 第三方库内部（D-G2 排除） | SQLite 内部虚拟机；CSV 导出模式下此项为零 |
| `dm_database_parser_sqllog::sqllog::Sqllog::parse_meta` | 5.9% | 第三方库内部（D-G2 排除） | 解析库内部元数据解析，非 src/ 函数 |
| `sqllog2db::cli::run::process_log_file` | 4.6% | self time < 5%（D-G1 第 1 条不满足） | 属于 src/cli/run.rs，但 4.6% < 5% 门控阈值 |
| `rayon_core::registry::WorkerThread::take_local_job` | 4.2% | 第三方库内部（D-G2 排除） | rayon 工作窃取调度，由解析库内部调用 |
| `memchr::memmem::searcher::searcher_kind_neon` | 4.1% | 第三方库内部 NEON SIMD（D-G2 排除） | memchr SIMD 字节搜索，由解析库调用 |
| `sqllog2db::features::replace_parameters::compute_normalized` | 3.2% | self time < 5%（D-G1 第 1 条不满足） | 属于 src/features/replace_parameters.rs，但 3.2% < 5% 门控阈值 |
| `rayon_core::join::join_context (closure)` | 3.0% | 第三方库内部（D-G2 排除） | rayon 并行 join 上下文，由解析库内部调用 |
| `serde_core::de::Visitor::visit_i128` | 2.6% | 第三方库内部（D-G2 排除） | serde 反序列化访客模式，非 src/ 函数 |

**结论：已达当前瓶颈.** 当前性能受限于：
- 第三方解析库（dm_database_parser_sqllog）内部 self time（D-G2 不可消除）
- 系统级内存分配与 mmap I/O（alloc / memchr 等）
- 流式 single-thread 架构的固有读-解析-写回串行依赖

依据 D-G3，本结论以本节段落形式签署，不另开 VERIFICATION.md。
PERF-10 验收通过：bench scenarios 已补全（D-B1），samply 已采集（D-P1/P2/P3），门控判定明确（D-G1 未命中），全量测试无回归（≤5% 容差）.

### 结论

- [x] D-B1 exclude_passthrough / exclude_active 两场景已补全
- [x] samply profile 已完成，top N 函数已记录（3129 个 CPU 采样，真实日志）
- [x] D-G1 门控判断已执行（无符合条件热点）
- [x] 无热点：已记录"已达当前瓶颈"结论
- [x] cargo test 全量通过，clippy/fmt 净化
- [x] PERF-10 验收通过：§当前瓶颈分析 逐项对照 D-G1/D-G2 完整签署

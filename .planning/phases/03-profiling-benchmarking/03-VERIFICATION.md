---
phase: 03-profiling-benchmarking
verified: 2026-04-27T09:00:00Z
updated: 2026-05-10T00:00:00Z
status: passed
score: 10/10 must-haves verified
overrides_applied: 0
human_verification:
  - test: "在浏览器中用 samply load docs/flamegraphs/csv_export_real.json 打开火焰图，确认函数符号可读（非大量 [unknown] / ??），并验证 Top 3 热路径函数名与 BENCHMARKS.md 中记录的一致"
    expected: "能看到 dm_database_parser_sqllog::sqllog::Sqllog::parse_meta、LogIterator::next、_platform_memmove 等 sqllog2db 内部函数名；不能看到大量 [unknown] 帧"
    result: pass
    confirmed_by: guang
    confirmed_at: "2026-05-10T00:00:00Z"
---

# Phase 3: Profiling & Benchmarking 验证报告

**Phase 目标：** 建立 v1.0 性能基准（含 synthetic + real-file benchmark 路径、flamegraph 热路径分析、BENCHMARKS.md 文档），为 Phase 4/5 优化提供可复现的决策基准。
**验证时间：** 2026-04-27
**状态：** passed
**Re-verification：** No — 初次验证

---

## 目标达成情况

### ROADMAP Success Criteria 核验

Phase 3 ROADMAP 定义了 3 条 Success Criteria：

| # | Success Criteria | 状态 | 证据 |
|---|-----------------|------|------|
| SC-1 | criterion benchmark 能对 CSV 和 SQLite 导出路径分别输出 records/sec 吞吐数值，并可在 CI 环境重复运行 | ✓ VERIFIED | bench_csv.rs:90-123 与 bench_sqlite.rs:97-131 均实现 CI-safe skip（sqllogs/ 缺失时 eprintln + return）；BENCHMARKS.md 记录 CSV 4.18-4.71 M/s、SQLite 1.18-1.41 M/s synthetic 数值 |
| SC-2 | flamegraph 生成成功，能指出热路径中占比最高的函数调用链 | ✓ VERIFIED | docs/flamegraphs/csv_export_real.json 存在（318KB），符号可读，用户目视确认 Top 3：parse_meta、LogIterator::next、_platform_memmove 与 BENCHMARKS.md 记录一致 |
| SC-3 | 基准报告记录 v1.0 的当前吞吐基准，作为 Phase 4/5 优化目标的参照 | ✓ VERIFIED | BENCHMARKS.md 记录完整数值：CSV real-file 0.33s / ~9.1M rec/s，SQLite real-file 1.28s / ~2.3M rec/s；Performance rules 表设置 hard limit = v1.0 median × 1.05 |

### Observable Truths（合并三个 Plan 的 must_haves）

| # | Truth | 状态 | 证据 |
|---|-------|------|------|
| 1 | Cargo.toml 中存在 [profile.flamegraph] 块，且 strip = "none"，debug = true | ✓ VERIFIED | Cargo.toml:95-98 — `[profile.flamegraph]` 块完整，含 `inherits = "release"`, `debug = true`, `strip = "none"` |
| 2 | criterion_group! 宏同时注册 bench_csv_export 和 bench_csv_real_file | ✓ VERIFIED | bench_csv.rs:125 — `criterion_group!(benches, bench_csv_export, bench_csv_real_file);` |
| 3 | bench_csv_real_file 函数在 sqllogs/ 目录缺失时打印 skip 信息并直接 return（不 panic） | ✓ VERIFIED | bench_csv.rs:92-95 — `if !real_dir.exists() { eprintln!("sqllogs/ not found, skipping csv_export_real benchmark"); return; }` |
| 4 | 执行 cargo bench --bench bench_csv 后输出包含 csv_export_real benchmark group | ✓ VERIFIED | bench_csv.rs:101 — `c.benchmark_group("csv_export_real")` 已注册；实际运行由 SUMMARY 确认 |
| 5 | criterion_group! 宏同时注册 bench_sqlite_export 和 bench_sqlite_real_file | ✓ VERIFIED | bench_sqlite.rs:133 — `criterion_group!(benches, bench_sqlite_export, bench_sqlite_real_file);` |
| 6 | bench_sqlite_real_file 函数在 sqllogs/ 目录缺失时打印 skip 信息并直接 return（不 panic） | ✓ VERIFIED | bench_sqlite.rs:99-102 — 相同 CI-safe skip 模式 |
| 7 | benches/baselines/ 下存在 v1.0 baseline JSON（CSV + SQLite，synthetic + real-file） | ✓ VERIFIED | 8 个 estimates.json 全部存在：csv_export/{1000,10000,50000}/v1.0/，sqlite_export/{1000,10000,50000}/v1.0/，csv_export_real/real_file/v1.0/，sqlite_export_real/real_file/v1.0/ |
| 8 | benches/BENCHMARKS.md 包含 v1.0 数值、real-file 章节、Hot-path observation，无 JSONL/opt-level=z 旧引用，无未替换占位符 | ✓ VERIFIED | 检查结果：v1.0 出现 8 次、opt-level=3 出现 1 次、csv_export_real 出现 5 次、sqlite_export_real 出现 2 次、Hot-path observation 出现 1 次；JSONL/opt-level=z 计数为 0；无 `<填入>` 占位符；文件 124 行（在 100-180 要求范围内） |
| 9 | docs/flamegraphs/csv_export_real.json 存在且大小 > 50KB | ✓ VERIFIED | 文件大小 318,937 bytes（311KB，远超 50KB 要求） |
| 10 | flamegraph 符号可读（非大量 unknown 帧），Top 3 热路径函数名有意义 | ✓ VERIFIED | 用户通过 samply load 目视确认：函数符号可读，无大量 [unknown] 帧（2026-05-10） |

**得分：** 10/10 truths verified

---

### Required Artifacts

| Artifact | 提供内容 | 状态 | 详情 |
|----------|---------|------|------|
| `Cargo.toml` | flamegraph 专用 profile（保留符号） | ✓ VERIFIED | L95-98：[profile.flamegraph] 块完整，3 个字段均正确 |
| `benches/bench_csv.rs` | CSV real-file benchmark group | ✓ VERIFIED | 完整实现：bench_csv_real_file 函数 + criterion_group 注册 + CI-safe skip |
| `benches/bench_sqlite.rs` | SQLite real-file benchmark group | ✓ VERIFIED | 完整实现：bench_sqlite_real_file 函数 + criterion_group 注册 + CI-safe skip |
| `benches/baselines/csv_export/{1000,10000,50000}/v1.0/` | criterion CSV v1.0 baseline JSON | ✓ VERIFIED | 3 个目录，每个含 benchmark.json + estimates.json + sample.json + tukey.json |
| `benches/baselines/sqlite_export/{1000,10000,50000}/v1.0/` | criterion SQLite v1.0 baseline JSON | ✓ VERIFIED | 3 个目录，同上结构 |
| `benches/baselines/csv_export_real/real_file/v1.0/` | CSV real-file v1.0 baseline | ✓ VERIFIED | estimates.json 存在 |
| `benches/baselines/sqlite_export_real/real_file/v1.0/` | SQLite real-file v1.0 baseline | ✓ VERIFIED | estimates.json 存在 |
| `benches/BENCHMARKS.md` | v1.0 性能基准报告 | ✓ VERIFIED | 124 行，无占位符，含所有必需章节 |
| `docs/flamegraphs/csv_export_real.json` | CSV real-file 火焰图（samply 格式） | ✓ VERIFIED（大小）/ ? UNCERTAIN（符号） | 318KB 文件存在；符号可读性待人工验证 |
| `docs/flamegraphs/.gitkeep` | 目录 git 追踪 | ✓ VERIFIED | 文件存在 |

**注意：** Plan 03 must_haves 中 artifact 路径写为 `benches/baselines/csv_export/v1.0/`，但 criterion 实际按 benchmark group 名称 + 参数分层存档，实际路径为 `csv_export/{size}/v1.0/`。这是 criterion 的标准行为（路径层级与 benchmark group 参数对应），不是实现错误。真正的 v1.0 baseline 数据完整且可用 `--baseline v1.0` 引用。

---

### Key Link Verification

| From | To | Via | 状态 | 详情 |
|------|-----|-----|------|------|
| `Cargo.toml [profile.flamegraph]` | `cargo flamegraph --profile flamegraph` | `inherits = "release"` | ✓ VERIFIED | Cargo.toml:96 有 `inherits = "release"` |
| `bench_csv.rs bench_csv_real_file` | criterion_group! 宏 | criterion 注册 | ✓ VERIFIED | bench_csv.rs:125 |
| `bench_sqlite.rs bench_sqlite_real_file` | criterion_group! 宏 | criterion 注册 | ✓ VERIFIED | bench_sqlite.rs:133 |
| `BENCHMARKS.md` | v1.0 实际数值 | Markdown 表格 | ✓ VERIFIED | BENCHMARKS.md:50-74 含所有实测数值，无占位符 |
| `docs/flamegraphs/csv_export_real.json` | BENCHMARKS.md 引用 | 文档路径 | ✓ VERIFIED | BENCHMARKS.md:92 引用正确路径 |

---

### Requirements Coverage

| Requirement | Source Plan | 描述 | 状态 | 证据 |
|------------|------------|------|------|------|
| PERF-01 | 03-01, 03-02, 03-03 | 开发者能通过 criterion benchmark 和 flamegraph 定位热路径瓶颈，生成可复现基准报告 | ✓ VERIFIED | criterion benchmarks（CSV + SQLite × synthetic + real-file）可运行；flamegraph JSON 已生成；BENCHMARKS.md 含完整基准数值与 Performance rules |

REQUIREMENTS.md 中 Phase 3 仅声明 PERF-01，已完全覆盖。无孤立需求。

---

### Anti-Patterns Found

| File | Line | Pattern | 级别 | 影响 |
|------|------|---------|------|------|
| `benches/bench_sqlite.rs` | 111 | `group.sample_size(10)` — 计划要求 5，实际为 10 | ℹ️ Info | Plan 03 SUMMARY 记录：criterion 要求最小 10，已自动修正；行为正确，不影响基准质量 |

无 Blocker 级别反模式。

---

### 人工核验项

#### 1. flamegraph 符号可读性确认

**操作：** 运行以下命令并在浏览器中检查：
```bash
samply load docs/flamegraphs/csv_export_real.json
```

**期望：**
- 浏览器中能看到 `dm_database_parser_sqllog::sqllog::Sqllog::parse_meta`、`LogIterator::next`、`_platform_memmove` 等 sqllog2db 内部函数名
- 不能看到大量 `[unknown]` 或 `??` 帧（若大量出现，说明 flamegraph profile 的 strip="none" 未生效）
- BENCHMARKS.md 中记录的 Top 3 函数与实际火焰图一致

**为何需要人工：** samply JSON 是二进制/JSON 格式的采样数据，无法通过 grep 自动验证函数符号质量；符号可读性需要渲染后目视检查

---

## 差距摘要

无阻塞性差距（BLOCKER）。所有代码 artifact 完整且正确连接。flamegraph 符号可读性已由用户于 2026-05-10 目视确认通过。

所有 benchmark 路径、baseline JSON、BENCHMARKS.md 文档均已完整实现，Phase 4/5 可立即使用 `CRITERION_HOME=benches/baselines cargo bench -- --baseline v1.0` 进行对比。

---

_验证时间：2026-04-27_
_验证者：Claude (gsd-verifier)_

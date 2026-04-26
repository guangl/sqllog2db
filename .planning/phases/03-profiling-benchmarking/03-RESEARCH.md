# Phase 3: Profiling & Benchmarking - Research

**Researched:** 2026-04-26
**Domain:** Rust criterion benchmarking + flamegraph profiling
**Confidence:** HIGH

---

## Summary

Phase 3 的目标是为 CSV 和 SQLite 导出路径建立可复现的性能基准，并通过 flamegraph 定位热路径瓶颈。研究发现项目已具备成熟的 criterion 基准测试基础设施（bench_csv.rs / bench_sqlite.rs / bench_filters.rs）和存档的 baseline JSON 数据（`benches/baselines/`），可以直接在此基础上扩展。

核心问题在于：现有合成 benchmark（~4.5-5.2M rec/s）与 real 1.1GB 文件实测（~1.55M rec/s）之间存在显著落差，需要增加 real-file benchmark 路径，并通过 flamegraph 明确瓶颈所在（解析层、格式化层、I/O 层）。macOS 环境已安装 `flamegraph 0.6.11`（依赖 dtrace）和 `samply 0.13.1`（基于 macOS Instruments 框架），两者均可使用。

**Primary recommendation:** 在现有 criterion 基础设施上增加 real-file benchmark，执行 flamegraph 采集并将 v1.0 基准数值写入 `benches/BENCHMARKS.md`，为 Phase 4/5 提供决策依据。

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PERF-01 | 开发者能够通过 criterion benchmark 和 flamegraph 定位 CSV 和 SQLite 导出的热路径瓶颈，并生成可复现的基准报告 | criterion 0.7 已配置 + flamegraph 0.6.11 已安装 + samply 0.13.1 已安装；需补充 real-file benchmark 和热路径 flamegraph |
</phase_requirements>

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Criterion benchmark 执行 | Build/CI | — | `cargo bench` 在开发机和 CI 均可运行，无运行时服务依赖 |
| Real-file throughput 测量 | Build/Dev | — | 需要访问 sqllogs/ 中的 1.1GB 真实日志；CI 中若无文件则跳过 |
| Flamegraph 生成 | Dev machine | — | 依赖 dtrace (macOS) 或 perf (Linux)，CI 一般无权限，仅在开发机执行 |
| Baseline 存档 | Version control | — | baseline JSON 提交到 `benches/baselines/`，跨 PR 可复现对比 |
| 基准报告更新 | Version control | — | `benches/BENCHMARKS.md` 记录数值，作为 Phase 4/5 优化目标参照 |

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| criterion | 0.7.0 | 统计驱动的 micro-benchmark | 项目已使用，baseline JSON 已存档 [VERIFIED: Cargo.lock] |
| flamegraph (cargo-flamegraph) | 0.6.11 | 生成 SVG flamegraph | 已安装，支持 `--bench` 参数 [VERIFIED: 环境探测] |
| samply | 0.13.1 | macOS 采样 profiler，输出 Firefox Profiler 格式 | 已安装，交互式 UI，补充 flamegraph [VERIFIED: 环境探测] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| inferno-flamegraph | (flamegraph 内置) | 将折叠栈转为 SVG | flamegraph 内部调用，无需单独安装 |
| dtrace | 系统内置 | macOS 内核级采样 | cargo-flamegraph 在 macOS 自动使用 |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| flamegraph (dtrace) | samply | samply 输出 Firefox Profiler 格式（交互式），flamegraph 输出静态 SVG（适合 CI 存档）；两者可并行使用 |
| criterion | divan | divan 更新但生态较小，项目已有 criterion 基础设施，无迁移必要 |

**Criterion 版本说明:** 项目使用 criterion 0.7.0（`^0.7`），crates.io 最新版为 0.8.2 [VERIFIED: crates.io API]。0.7.0 已完全满足需求（`--save-baseline` / `--baseline` / `Throughput::Elements`），**不需要升级**。

---

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                  Phase 3 Profiling Flow                     │
└─────────────────────────────────────────────────────────────┘

  cargo bench --bench bench_csv
       │
       ├─► synthetic (1K/10K/50K records)
       │       └─► handle_run() → CSV → /dev/null
       │               └─► criterion: thrpt (Melem/s)
       │
       └─► [NEW] real-file (sqllogs/ 269MB × 2 files)
               └─► handle_run() → CSV → /dev/null
                       └─► timing script OR criterion bench_csv_real

  cargo bench --bench bench_sqlite
       └─► synthetic (same sizes)
               └─► handle_run() → SQLite → bench.db
                       └─► criterion: thrpt (Melem/s)

  cargo flamegraph --bench bench_csv -- --profile-time 10
       └─► dtrace sampling → folded stacks → SVG
               └─► flamegraph.svg (热路径可视化)

  samply record cargo bench --bench bench_csv -- --profile-time 10
       └─► macOS Instruments → prof.json
               └─► 浏览器交互式查看
```

### Recommended Project Structure
```
benches/
├── bench_csv.rs          # 已有：合成 benchmark（扩展 real-file group）
├── bench_sqlite.rs       # 已有：合成 benchmark
├── bench_filters.rs      # 已有：filter 管线 benchmark
├── BENCHMARKS.md         # 更新：记录 v1.0 基准数值 + 测量方法
└── baselines/
    ├── csv_export/       # 已有：criterion baseline JSON
    ├── sqlite_export/    # 已有：criterion baseline JSON
    ├── filters/          # 已有
    └── report/           # 已有

docs/flamegraphs/         # [NEW] 存档 flamegraph SVG（gitignore 或提交小文件）
```

### Pattern 1: Criterion Baseline 工作流
**What:** 用 `--save-baseline` 存档当前数值，用 `--baseline` 对比回归
**When to use:** 每次优化迭代后

```bash
# Source: benches/BENCHMARKS.md + criterion 文档
# 存档 v1.0 基准
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --save-baseline v1.0

# 优化后对比
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0
```

输出示例：
```
csv_export/10000  time: [2.09 ms 2.09 ms 2.10 ms]
                  change: [-5.2% -4.8% -4.4%] (p = 0.00 < 0.05)
                  Performance has improved.
```

### Pattern 2: Criterion `--profile-time` + flamegraph
**What:** 用 criterion 的 profiler-friendly 模式配合 cargo-flamegraph
**When to use:** 采集 flamegraph 时，避免 criterion 统计逻辑干扰符号

```bash
# Source: cargo-flamegraph 文档 + criterion --help
# macOS 需要 sudo（dtrace 权限）
sudo cargo flamegraph --bench bench_csv -- --profile-time 10

# 或者用 samply（无需 sudo，但需先 codesign）
samply setup -y
samply record cargo bench --bench bench_csv -- --profile-time 10
```

### Pattern 3: Real-file Benchmark（Timing Script 方案）
**What:** 直接计时 `cargo run --release` 处理真实日志文件
**When to use:** criterion 合成 benchmark 无法替代真实文件 I/O 特性时

```bash
# 简单计时脚本（不需要新 bench 文件）
time cargo run --release -- run -c config.toml 2>/dev/null
# 或者通过 hyperfine 多次采样
hyperfine --warmup 2 'cargo run --release -- run -c config.toml'
```

**替代方案：在 bench_csv.rs 中增加 `csv_export_real` group**，读取 `sqllogs/` 下真实文件。但需要注意：CI 环境可能无真实文件，需用 `#[cfg]` 或环境变量控制跳过。

### Anti-Patterns to Avoid
- **在 bench 中不设 `quiet=true`:** 进度条 I/O 会污染计时（现有代码已正确处理）
- **SQLite bench 写 /dev/null:** SQLite 需要真实文件（现有代码已正确用 bench.db）
- **flamegraph 不用 `--profile-time`:** 直接运行 criterion 会包含统计框架开销，掩盖真实热路径
- **release profile 含 `strip = "symbols"`:** flamegraph 需要符号，须用 `[profile.bench]` 或 `--dev` 配置覆盖（见下方 Pitfall 2）

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 统计显著性判断 | 自己比较时间差 | criterion `--baseline` | criterion 用 t-test / Mann-Whitney 控制噪声 |
| 火焰图生成 | 手动 dtrace + awk | cargo-flamegraph | 封装了 dtrace/perf + inferno 全流程 |
| 多次采样稳定性 | sleep + 手动平均 | criterion warm-up + sample_size | criterion 自动 warm-up 和 outlier 剔除 |
| real-file 多次计时 | shell loop | hyperfine | 自动 warm-up、统计摘要、CI-friendly 输出 |

**Key insight:** Criterion 的价值不在于速度，而在于统计可靠性。`--baseline` 对比能区分"真实提升"和"噪声波动"，是 Phase 4/5 优化决策的基础。

---

## Common Pitfalls

### Pitfall 1: Real-file vs Synthetic 吞吐落差
**What goes wrong:** 合成 benchmark 显示 ~4.5M rec/s，实测 real 文件只有 ~1.55M rec/s，差 3x。如果只看合成数据，会对优化目标产生误判。
**Why it happens:** 合成数据行长固定（~170 B），内存局部性好；真实日志行长变化大，SQL 更复杂，解析器负担更重，I/O 读取模式不同。
**How to avoid:** 必须建立 real-file 基准（使用 `sqllogs/` 中的 269MB × 2 文件），与合成 benchmark 并列记录。
**Warning signs:** Phase 4 优化后合成 benchmark 提升但 real-file 未提升，说明优化了错误路径。

### Pitfall 2: Release profile 含 `strip = "symbols"` 导致 flamegraph 无符号
**What goes wrong:** `Cargo.toml` 的 `[profile.release]` 配置了 `strip = "symbols"`，cargo-flamegraph 默认用 release profile，生成的火焰图全是 `unknown`。
**Why it happens:** `strip` 在链接后删除调试符号，flamegraph 无法解析函数名。
**How to avoid:** 为 bench profile 单独配置，或用 `--profile` 指定不 strip 的 profile：
```toml
# Cargo.toml
[profile.flamegraph]
inherits = "release"
debug = true
strip = "none"
```
```bash
cargo flamegraph --profile flamegraph --bench bench_csv -- --profile-time 10
```
[VERIFIED: cargo-flamegraph README + Cargo.toml 当前配置]

### Pitfall 3: macOS dtrace 需要 sudo（或 SIP 限制）
**What goes wrong:** `cargo flamegraph` 在 macOS 上提示权限不足，无法 attach dtrace。
**Why it happens:** macOS SIP (System Integrity Protection) 限制 dtrace 对非 root 进程的采样。
**How to avoid:** 使用 `sudo cargo flamegraph`；或改用 `samply`（codesign 后无需 sudo，且对 macOS 更友好）。
**Warning signs:** `dtrace: failed to initialize consumer` 错误。

### Pitfall 4: Criterion baseline JSON 路径冲突
**What goes wrong:** 直接运行 `cargo bench` 会覆盖 `target/criterion/` 下的数据，而不是 `benches/baselines/`。
**Why it happens:** criterion 默认 home 是 `target/criterion`；`CRITERION_HOME` 环境变量控制路径。
**How to avoid:** 所有基准存档命令必须加 `CRITERION_HOME=benches/baselines`，CI 脚本同理。

---

## Code Examples

### 新增 real-file benchmark group（bench_csv.rs 扩展）
```rust
// Source: 基于现有 bench_csv.rs 模式扩展
fn bench_csv_real_file(c: &mut Criterion) {
    let real_dir = PathBuf::from("sqllogs");
    if !real_dir.exists() {
        eprintln!("sqllogs/ not found, skipping real-file benchmark");
        return;
    }
    // 统计真实文件总记录数（用于 Throughput 计算）
    // 注意：记录数需提前测量并硬编码，或在 setup 阶段扫描
    let cfg = make_config(&real_dir, &PathBuf::from("target/bench_csv_real"));
    let mut group = c.benchmark_group("csv_export_real");
    group.sample_size(10); // 真实文件慢，减少采样
    group.measurement_time(std::time::Duration::from_secs(60));
    // throughput 需提前知道记录数，或省略（只记录时间）
    group.bench_function("1.1GB_real", |b| {
        b.iter(|| {
            handle_run(&cfg, None, false, true,
                &Arc::new(AtomicBool::new(false)), 80, false, None, 1).unwrap();
        });
    });
    group.finish();
}
```

### 不 strip 符号的 flamegraph profile 配置
```toml
# Cargo.toml — 新增 profile
[profile.flamegraph]
inherits = "release"
debug = true      # 保留 DWARF 符号
strip = "none"    # 不删除符号
```

### flamegraph 采集命令
```bash
# macOS (dtrace + sudo)
sudo cargo flamegraph --profile flamegraph --bench bench_csv \
  -- --profile-time 15 csv_export/10000

# 使用 samply（无需 sudo，codesign 后）
samply setup -y
samply record cargo bench --bench bench_csv \
  -- --profile-time 15 csv_export/10000
```

### 存档并对比 v1.0 baseline
```bash
# Step 1: 记录 v1.0 基准
CRITERION_HOME=benches/baselines \
  cargo bench --bench bench_csv -- --save-baseline v1.0
CRITERION_HOME=benches/baselines \
  cargo bench --bench bench_sqlite -- --save-baseline v1.0

# Step 2: Phase 4/5 优化后对比
CRITERION_HOME=benches/baselines \
  cargo bench --bench bench_csv -- --baseline v1.0
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 手动 time 命令计时 | criterion 统计 benchmark | — | 统计显著性，可重复对比 |
| perf (Linux-only) | cargo-flamegraph (跨平台) + samply (macOS) | — | macOS 开发者可本地生成 flamegraph |
| criterion 0.5.x (黑盒 benchmark) | criterion 0.7.x (`--profile-time` 模式) | — | 可配合 profiler 精确采样，不含统计开销 |

**已知状态：**
- `benches/baselines/` 已有 v0.5.0（2026-04-04）时期的 baseline JSON，但 BENCHMARKS.md 中记录的是该版本数值
- 当前 v0.10.7 可能与存档 baseline 不直接可比（代码变化），需重新采集 v1.0 基准

---

## Runtime State Inventory

> 本 Phase 为纯 benchmark/profiling 代码变更，无数据迁移或重命名，跳过此节。

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo-flamegraph | PERF-01 flamegraph | ✓ | 0.6.11 | samply |
| samply | macOS profiling | ✓ | 0.13.1 | cargo-flamegraph + sudo |
| dtrace | cargo-flamegraph (macOS) | ✓ | 系统内置 | samply (无需 dtrace) |
| sqllogs/ 真实日志文件 | real-file benchmark | ✓ | 269MB × 2 文件 | 跳过 real-file bench（仅合成）|
| criterion 0.7.0 | 所有 benchmark | ✓ | 0.7.0 (Cargo.lock) | — |
| hyperfine | real-file 计时脚本 | [ASSUMED] 未验证 | — | `time` 命令 |

**需要注意：**
- `sudo` 权限：`cargo flamegraph` 在 macOS 需要 sudo，或改用 samply（codesign 后无需 sudo）
- CI 环境：flamegraph 通常无法在 CI 执行（无 dtrace 权限），real-file benchmark 也因文件缺失而跳过 — **这两点均属预期**，不影响 PERF-01 的"可在 CI 重复运行"要求（criterion 合成 benchmark 满足 CI 要求）

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | criterion 0.7.0 |
| Config file | Cargo.toml (`[[bench]]` 条目) |
| Quick run command | `cargo bench --bench bench_csv -- --sample-size 10 csv_export/10000` |
| Full suite command | `CRITERION_HOME=benches/baselines cargo bench -- --save-baseline v1.0` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PERF-01 | criterion 输出 CSV records/sec | benchmark | `cargo bench --bench bench_csv` | ✅ benches/bench_csv.rs |
| PERF-01 | criterion 输出 SQLite records/sec | benchmark | `cargo bench --bench bench_sqlite` | ✅ benches/bench_sqlite.rs |
| PERF-01 | flamegraph SVG 生成成功 | manual | `sudo cargo flamegraph --profile flamegraph --bench bench_csv -- --profile-time 10` | ✅ (工具已装，profile 待配置) |
| PERF-01 | v1.0 基准数值记录在 BENCHMARKS.md | documentation | 人工核查 BENCHMARKS.md | ✅ 待更新 |

### Sampling Rate
- **Per task commit:** `cargo bench --bench bench_csv -- --sample-size 10 csv_export/10000`（快速验证 benchmark 可运行）
- **Per wave merge:** `CRITERION_HOME=benches/baselines cargo bench -- --save-baseline v1.0`（完整采集）
- **Phase gate:** BENCHMARKS.md 更新 + flamegraph SVG 生成成功

### Wave 0 Gaps
- [ ] `[profile.flamegraph]` 配置块需添加到 Cargo.toml（`strip = "none"`, `debug = true`）
- [ ] real-file benchmark group `bench_csv_real` / `bench_sqlite_real` — 覆盖 PERF-01 "real-file 基准"要求

---

## Project Constraints (from CLAUDE.md)

| Directive | Impact on Phase 3 |
|-----------|------------------|
| `cargo clippy --all-targets -- -D warnings` 必须零警告 | 新增 bench 代码必须通过 clippy |
| `cargo fmt` | 新增代码必须格式化 |
| 函数不超过 40 行 | real-file benchmark helper 函数需注意拆分 |
| `cargo test` | benchmark 不替代 unit test，但 bench 文件本身须可编译通过 |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | hyperfine 未在当前环境验证是否已安装 | Environment Availability | 若无 hyperfine，real-file 计时用 `time` 命令替代，影响较小 |
| A2 | samply codesign 状态未验证（`samply setup` 是否已执行过） | Environment Availability | 若未 codesign，samply record 失败，退回 sudo flamegraph |
| A3 | CI 环境无 dtrace 权限（基于 macOS CI 常规配置推断） | Environment Availability | 若 CI 有 dtrace 权限，可将 flamegraph 生成加入 CI，但不影响 PERF-01 达成 |

---

## Open Questions

1. **Real-file benchmark 的记录数量**
   - What we know: sqllogs/ 有 269MB × 2 = 538MB 日志文件（比 README 中提到的 1.1GB 少一半）
   - What's unclear: 是否还有其他日志文件？记录数需要预先扫描才能设置 `Throughput::Elements`
   - Recommendation: Plan 中安排"扫描记录数"任务，或 real-file bench 省略 throughput，仅记录绝对时间

2. **BENCHMARKS.md 中的 baseline 时间点**
   - What we know: `benches/baselines/` 存档的是 v0.5.0（2026-04-04）的数据，当前版本为 v0.10.7
   - What's unclear: v1.0 基准应该重新采集还是沿用旧数据
   - Recommendation: 重新采集（`--save-baseline v1.0`），旧数据仅供参考；BENCHMARKS.md 明确标注版本

---

## Sources

### Primary (HIGH confidence)
- `Cargo.toml` / `Cargo.lock` — criterion 0.7.0 版本、dev-dependencies 配置
- `benches/bench_csv.rs` / `bench_sqlite.rs` / `bench_filters.rs` — 现有 benchmark 结构
- `benches/BENCHMARKS.md` — 已记录的 baseline 数值和对比命令
- `benches/baselines/` — criterion baseline JSON 数据
- 环境探测 (`command -v flamegraph`, `flamegraph --version`, `samply --version`) — 工具可用性
- `src/exporter/mod.rs` / `csv.rs` / `sqlite.rs` — 热路径代码结构

### Secondary (MEDIUM confidence)
- crates.io API (`/api/v1/crates/criterion`) — criterion 最新版 0.8.2 [VERIFIED]
- cargo-flamegraph `--help` — `--bench` 参数支持，`--profile` 参数

### Tertiary (LOW confidence)
- samply codesign 状态（仅验证版本，未验证 setup 状态）[ASSUMED]

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — criterion 0.7.0 已锁定在 Cargo.lock，flamegraph/samply 均已环境验证
- Architecture: HIGH — 现有 bench 结构完整，扩展路径明确
- Pitfalls: HIGH — strip symbols 坑和 CRITERION_HOME 坑均通过代码/文档直接验证

**Research date:** 2026-04-26
**Valid until:** 2026-05-26（criterion 0.7 稳定，flamegraph 0.6.x 稳定，30 天内不会过期）

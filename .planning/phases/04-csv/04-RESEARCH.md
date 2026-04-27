# Phase 4: CSV 性能优化 - Research

**Researched:** 2026-04-27
**Domain:** Rust 热路径优化 — 解析层减少分配、CSV 格式化、BufWriter I/O
**Confidence:** HIGH（代码库直接读取 + 官方 Rust 文档支持）

---

## Summary

Phase 3 flamegraph 已确认三条热路径：`parse_meta`（最高占比）、`LogIterator::next`、`_platform_memmove`。结合代码审计发现：**parse_meta 本身来自上游 `dm-database-parser-sqllog` 1.0.0 crate，其代码逻辑高度优化（`Cow::Borrowed` 零分配路径已启用），热点并非格式化层问题，而是解析层的字符串拷贝**。CSV exporter 层（`write_record_preparsed`）已相当精简：16MB BufWriter、预分配 `line_buf`、memchr 转义、`itoa`。当前 synthetic benchmark 吞吐 4.18–4.71 M/s，real-file ~9.1 M rec/s（含 I/O）。

目标是相比 Phase 3 baseline（`csv_export/10000` median = 2.127ms）提升 ≥10%（即 ≤1.91ms）。

**可动手的优化点经代码分析后确认有三处，按 ROI 排序：**

1. **`parse_performance_metrics()` 在每条记录上被调用一次**（`cli/run.rs:175` 的热循环），返回 `PerformanceMetrics<'a>`。其中 `find_indicators_split()` 会触发反向扫描（memrchr 循环）+条件验证，是 _platform_memmove 的来源之一。可以通过增加 `bench_csv_format_only` micro-benchmark（只测格式化，不含解析）来隔离开销。

2. **`compute_normalized` 中存在一处每条记录的两次 `CompactString::from` 分配**（trxid + statement key），即使大多数 SQL 无占位符也会触发 `count_placeholders` 之后的 key 创建（`replace_parameters.rs:384–387`）。当前代码已有"无占位符则早返回"优化，key 创建只在有占位符时执行——此处实际已无 hot path 问题。

3. **`write_record_preparsed` 的 `line_buf.reserve(120 + sql_len + ns_len + 8)` 每条记录调用一次**。虽然 clear() 保留容量，但 `reserve()` 内部仍需一次容量检查和条件分支。对超短 SQL 可能触发不必要的 reserve。

**Primary recommendation:** 添加一个专门的 CSV 格式化 micro-benchmark（`bench_csv_format_only`），孤立格式化路径开销，并与此同时通过修改 `write_record_preparsed` 的 reserve 策略（使用 `try_reserve` + 只在容量不足时 reserve）减少分支，以及将 `BufWriter` 容量从 16MB 调整实验看是否对 real-file 场景有益。

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 热循环驱动（record 迭代） | `cli/run.rs` | — | `process_log_file` 的 `for result in parser.iter()` 循环 |
| 解析（parse_meta / parse_performance_metrics） | 上游 crate | — | `dm-database-parser-sqllog 1.0.0` 负责解析，sqllog2db 无法修改 |
| 管线过滤（Pipeline） | `features/mod.rs` | `cli/run.rs` | `pipeline.run_with_meta()` 在记录进入 exporter 前执行 |
| CSV 格式化 + 序列化 | `exporter/csv.rs` | — | `write_record_preparsed` 负责所有字段拼接 |
| I/O 缓冲 | `exporter/csv.rs` | OS | `BufWriter<File>` 16MB 缓冲 + `write_all` |
| 参数替换（normalized_sql） | `features/replace_parameters.rs` | `cli/run.rs` | `compute_normalized` 在 exporter 调用前执行 |

---

## Standard Stack

### Core（已在项目中使用）

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `itoa` | 1.0 | 整数零分配 ASCII 序列化 | 已启用，比 `format!` 快 3-5x [VERIFIED: Cargo.toml] |
| `memchr` | 2 | SIMD 字节搜索 | 已用于 `write_csv_escaped` 和 `replace_parameters` [VERIFIED: src/] |
| `compact_str` | 0.9 | ≤24 字节字符串内联存储 | 已用于 `ParamBuffer` key，消除短字符串堆分配 [VERIFIED: Cargo.toml] |
| `smallvec` | 1 | ≤6 参数列表内联存储 | 已用于 `ParamBuffer` value [VERIFIED: Cargo.toml] |
| `mimalloc` | 0.1 | 全局分配器 | 已注册为 `#[global_allocator]`，对小对象分配比系统 malloc 快 [VERIFIED: src/main.rs] |
| `criterion` | 0.7 | benchmark 框架 | Phase 3 已建立基础设施 [VERIFIED: Cargo.toml] |

### Supporting（可以引入）

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `ryu` | 1 | f32/f64 零分配浮点序列化 | 若需要输出 exectime 为精确浮点字符串（当前用 itoa + f32→i64 转换，无需 ryu）[VERIFIED: Cargo.toml 已有] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `write_all(line_buf)` | `io::Write::write_vectored` | scatter/gather IO 在大块连续写入时无优势，BufWriter 已足够 [ASSUMED] |
| `Vec<u8>` line_buf | `bytes::BytesMut` | 无显著优势，bytes crate 增加依赖开销 [ASSUMED] |
| 16MB BufWriter | 64KB–1MB BufWriter | 对于单线程顺序写入，16MB vs 1MB 吞吐差异极小（OS page cache 负责缓冲）[ASSUMED] |

**Installation:** 无需新增依赖，所有需要的 crate 已在 `Cargo.toml` 中。

---

## Architecture Patterns

### System Architecture Diagram

```
日志文件 (sqllogs/*.log)
    ↓
LogParser::iter() — dm-database-parser-sqllog 1.0.0 流式解析
    ↓ Sqllog<'_> (Cow::Borrowed — 零拷贝引用原始 mmap 字节)
    ↓
process_log_file() 热循环 [cli/run.rs]
    ├── parse_meta() → MetaParts<'_>    ← 热点 #1 (flamegraph top 1)
    ├── pipeline.run_with_meta()         ← 可选，有过滤器时启用
    ├── parse_performance_metrics()      ← 热点 #2 (find_indicators_split 含 memrchr)
    ├── compute_normalized()             ← 可选，do_normalize 时启用
    └── exporter_manager.export_one_preparsed()
            ↓
        write_record_preparsed() [exporter/csv.rs]   ← 格式化层
            ├── line_buf.clear() + reserve()
            ├── extend_from_slice × 10–15 字段
            ├── write_csv_escaped() (memchr SIMD)
            └── BufWriter::write_all(line_buf)      ← 16MB 缓冲写入
                    ↓
                /dev/null 或真实 CSV 文件
```

### Recommended Project Structure

```
src/
├── exporter/csv.rs         # CSV 格式化热路径 — 优化重点
├── features/
│   └── replace_parameters.rs  # compute_normalized — 次要检查点
└── cli/run.rs              # 热循环 orchestration
benches/
├── bench_csv.rs            # 新增 bench_csv_format_only group
└── baselines/
    └── csv_export/*/v1.0/  # Phase 3 baseline，Phase 4 用 --baseline v1.0 对比
```

### Pattern 1: Micro-benchmark 格式化路径隔离

**What:** 新增一个只测格式化（不含解析）的 benchmark，直接调用 `write_record_preparsed`。
**When to use:** 验证 csv.rs 内部优化效果，排除解析层噪声。

```rust
// benches/bench_csv.rs 新增 group
fn bench_csv_format_only(c: &mut Criterion) {
    use dm_database_sqllog2db::exporter::csv::CsvExporter;
    // 预先解析好 meta + pm，在 b.iter() 内只跑格式化
    // 此 benchmark 直接量化 write_record_preparsed 的吞吐
    let mut group = c.benchmark_group("csv_format_only");
    group.throughput(Throughput::Elements(10_000));
    // ...
    group.finish();
}
```

注意：`write_record_preparsed` 目前是私有的（`fn`），需要改为 `pub(crate)` 或通过公开的 `export_one_preparsed` 路径测试。

### Pattern 2: Criterion `--baseline v1.0` 对比

**What:** Phase 4 所有优化完成后，用 Phase 3 建立的 baseline 进行对比验证。

```bash
# 对比 Phase 4 修改与 v1.0 baseline
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0

# 保存 Phase 4 结果
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --save-baseline phase4
```

criterion 会自动输出 "Performance has improved" / "No change" / "Performance has regressed"。

### Anti-Patterns to Avoid

- **过度 reserve：** 对每条记录调用 `reserve(120 + sql_len + ns_len + 8)` 时，若 `line_buf` 已有足够容量，这次 reserve 仍会发生一次 capacity 检查。可以考虑只在 `len() > capacity() / 2` 时才 reserve，但需 benchmark 验证确实有收益再提交。
- **引入新分配：** 不要为"代码清洁"引入中间 `String` 或 `Vec` 转换，所有字段写入必须保持 `extend_from_slice` 路径。
- **修改解析库行为：** `parse_meta` 和 `parse_performance_metrics` 来自上游 crate，不能直接修改。若要减少调用，需在热循环中重构调用时序。

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 整数序列化 | `format!("{}", n)` | `itoa::Buffer::format()` | 已使用；format! 触发堆分配 [VERIFIED: csv.rs] |
| 字节搜索 | 手写字节循环 | `memchr::memchr()` | 已使用；SIMD 加速，单次调用跳过大段无关字节 [VERIFIED: csv.rs] |
| 性能基准 | 手写计时代码 | `criterion` | 已有基础设施；criterion 统计正确，减少测量噪声 [VERIFIED: bench_csv.rs] |
| 全局分配器 | jemalloc 手动集成 | `mimalloc` | 已注册；mimalloc 对小对象分配有优势 [VERIFIED: main.rs] |

**Key insight:** CSV 热路径已非常成熟，优化空间在于精准量化而非大幅重构——先用 micro-benchmark 确认瓶颈，再做精准改动。

---

## Common Pitfalls

### Pitfall 1: 混淆 synthetic 与 real-file benchmark 的提升含义

**What goes wrong:** 只看 synthetic benchmark 提升 10%，忽略 real-file 提升可能不同（解析库开销比例更高）。
**Why it happens:** synthetic benchmark 只测一种记录格式，real-file 含多种格式和长 SQL。
**How to avoid:** 成功标准要求 real-file criterion benchmark 相比 Phase 3 baseline 提升 ≥10%（以 median time 为准）。两个数字都要汇报。
**Warning signs:** synthetic 提升 >10% 但 real-file 几乎无变化，说明优化命中了 synthetic 特有的路径。

### Pitfall 2: `line_buf.reserve()` 的实际作用被高估

**What goes wrong:** 以为 reserve 是热路径瓶颈，花大量时间优化它，实际收益微乎其微（reserve 是 `O(1)` 容量检查，真正开销来自内存拷贝）。
**Why it happens:** 不熟悉 Rust Vec 实现细节——`reserve()` 在容量足够时是单次无分支检查，几乎无成本。
**How to avoid:** 先 micro-benchmark 验证 reserve 是否出现在火焰图，再考虑优化。
**Warning signs:** 修改 reserve 策略后 benchmark 变化 < 1%（在噪声范围内）。

### Pitfall 3: 误将解析层开销归入格式化层

**What goes wrong:** flamegraph 显示 `parse_meta` 是热点，误以为可以通过优化 `csv.rs` 消除。
**Why it happens:** `parse_meta` 发生在 `cli/run.rs` 热循环中，而非 csv.rs 内部。
**How to avoid:** 代码路径已明确：`cli/run.rs:162–165` 在进入 exporter 前调用 `parse_meta`。可以的优化方向是减少每条记录的 `parse_performance_metrics()` 调用次数（如缓存或延迟解析），但上游 crate 的 `find_indicators_split` 是不可避免的成本。
**Warning signs:** 在 csv.rs 大量修改后 flamegraph 热点位置不变。

### Pitfall 4: Criterion baseline 路径层级理解错误

**What goes wrong:** 保存 Phase 4 baseline 时路径与 Phase 3 不一致，导致 `--baseline v1.0` 找不到对比数据。
**Why it happens:** Criterion 按 `{bench_name}/{group}/{parameter}/` 层级存档，参数名必须完全一致。
**How to avoid:** 新增的 `csv_format_only` group 直接存档到新路径（不与 v1.0 对比），只有 `csv_export` 和 `csv_export_real` group 需要与 v1.0 baseline 对比。
**Warning signs:** `--baseline v1.0` 运行时输出 "No baseline found for ..."。

---

## Code Examples

### 当前热路径核心（已验证，勿破坏）

```rust
// Source: src/exporter/csv.rs:91-144
// 全量字段路径：直接 extend_from_slice，无分支
line_buf.clear();
line_buf.reserve(120 + sql_len + ns_len + 8);

line_buf.extend_from_slice(sqllog.ts.as_ref().as_bytes());
line_buf.push(b',');
line_buf.extend_from_slice(itoa_buf.format(meta.ep).as_bytes());
// ... 其余字段
line_buf.push(b'\n');
writer.write_all(line_buf)
```

```rust
// Source: src/exporter/csv.rs:13-21
// write_csv_escaped：memchr SIMD 跳过无引号内容
#[inline]
fn write_csv_escaped(buf: &mut Vec<u8>, bytes: &[u8]) {
    let mut remaining = bytes;
    while let Some(pos) = memchr::memchr(b'"', remaining) {
        buf.extend_from_slice(&remaining[..=pos]);
        buf.push(b'"');
        remaining = &remaining[pos + 1..];
    }
    buf.extend_from_slice(remaining);
}
```

### Criterion Micro-benchmark 格式化路径隔离（建议新增）

```rust
// benches/bench_csv.rs 中新增 group
fn bench_csv_format_only(c: &mut Criterion) {
    // 预先构造好 Sqllog 记录（解析完成），在 iter() 内只测格式化
    // 目标：量化 write_record_preparsed 在 10k 记录上的纯格式化吞吐
    // 需要 CsvExporter::write_record_preparsed 可访问（pub(crate)）
}
```

### Criterion 对比命令（Phase 4 验证）

```bash
# 验证相对 v1.0 的提升
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0

# 保存 Phase 4 baseline（phase4 completed）
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --save-baseline phase4

# 生成 Phase 4 flamegraph（对比 Phase 3）
samply record --save-only --output docs/flamegraphs/csv_export_real_phase4.json -- \
  cargo bench --profile flamegraph --bench bench_csv \
  -- --profile-time 15 csv_export_real/real_file
```

---

## Phase Requirements → Research Support

<phase_requirements>

| ID | Description | Research Support |
|----|-------------|------------------|
| PERF-02 | CSV 导出吞吐在 real 1.1GB 日志文件上相比 v1.0 基准（~1.55M records/sec）有可量化提升（目标 ≥10%） | Phase 3 建立了 `csv_export_real/real_file` baseline（median=0.33s），可用 `--baseline v1.0` 验证；热路径分析指向 parse_performance_metrics + line_buf 格式化 |
| PERF-03 | CSV 格式化/序列化路径优化（减少字符串分配、改进 buffer 策略或利用更快的格式化 API） | 代码审计确认 `write_record_preparsed` 已使用 itoa/memchr/BufWriter；新增 `bench_csv_format_only` micro-benchmark 可孤立格式化开销；reserve 策略和 line_buf 初始化可调优 |
| PERF-08 | 热循环内减少堆分配（SmallVec / compact_str 等已有 crate 的充分利用，或消除隐藏 clone） | 代码审计显示：`compute_normalized` 中 `CompactString::from` 只在有占位符的记录上发生（已有早退优化）；主要可查点是 `write_record_preparsed` 的 `path.to_path_buf()` 错误路径（非热路径）和 `ExportStats::record_success` 的 usize 递增（无分配）；flamegraph Phase 4 对比是唯一可靠的"显著减少"验证手段 |

</phase_requirements>

---

## Hot Path Deep Analysis（关键发现）

### 发现 1：CSV exporter 格式化层已高度优化

经过代码审计，`write_record_preparsed`（`csv.rs:78–258`）满足：

- `line_buf.clear()` 保留容量，无堆分配（已分配容量 ≥ 2KB）
- `extend_from_slice` 直接写入 `Vec<u8>`，无中间 `String` 转换
- `itoa::Buffer` 复用（不含分配）
- `write_csv_escaped` 使用 memchr SIMD
- `BufWriter::write_all` 写入 16MB 缓冲区，单次系统调用频率极低
- `ExporterKind` 是静态枚举分发（非 `Box<dyn>`），编译器可内联

**结论：** 格式化层本身"已经很快"，进一步提升需要 micro-benchmark 量化后再决策。

### 发现 2：隐藏的 clone 位置

审计 `cli/run.rs` 热循环（`process_log_file`，`'outer` 循环内），实际上没有发现隐藏的 `clone()`。关键路径：

```rust
// cli/run.rs:159–209 热循环内（每条 record 执行）
let (passes, cached_meta) = if pipeline.is_empty() {
    (true, None)
} else {
    let meta = record.parse_meta();  // 解析，非 clone
    let ok = pipeline.run_with_meta(&record, &meta);
    (ok, Some(meta))
};
// ... 下面复用 meta，无额外 clone
```

**结论：** `_platform_memmove` 的来源是 `parse_meta` 内部的字符串字节拷贝（`Cow::Owned` 路径），以及 `parse_performance_metrics` 的 `find_indicators_split()` 内存扫描。这是解析层无法绕开的成本，但可以通过减少不必要的 `parse_performance_metrics()` 调用来优化。

### 发现 3：`parse_performance_metrics` 调用可能被优化

`cli/run.rs:176`：

```rust
// 当前：passes=true 时总是调用 parse_performance_metrics
let pm = record.parse_performance_metrics();
```

上游 `parse_performance_metrics()` 内部调用一次 `find_indicators_split()`（memrchr 反向扫描），这是 flamegraph top-3 热路径之一。**此处已无重复调用**（`parse_meta` 和 `parse_performance_metrics` 分别扫描不同区段），但 `find_indicators_split` 本身在有性能指标的记录上会扫描最多 256 字节。

### 发现 4：`compute_normalized` 中的 key 分配

`replace_parameters.rs:379–387`（有占位符时创建 key）：

```rust
let (placeholder_count, detected_colon) = count_placeholders(pm_sql);
if placeholder_count == 0 {
    return None;  // 大多数 SQL 走此路径，无 CompactString 分配
}
let key = (
    CompactString::from(meta.trxid.as_ref()),    // 分配（≤23 字节内联）
    CompactString::from(meta.statement.as_ref()), // 分配（≤23 字节内联）
);
let params = buffer.remove(&key)?;
```

对于有占位符的 DML 记录，每次导出产生 2 次 `CompactString` 分配（内联存储，无堆分配）+ 1 次 `ahash::HashMap::remove`。这个成本在真实日志中（PARAMS 记录比例通常 <10%）是可接受的。

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `format!("{}", field)` 格式化 | `itoa::Buffer` + `extend_from_slice` | v1.0 已完成 | 3-5x 整数序列化提速 |
| `Box<dyn Exporter>` 虚表分发 | `ExporterKind` 枚举静态分发 | v1.0 已完成 | 允许编译器内联热路径 |
| 每条记录 parse_meta × 2 | parse_meta 共享给 pipeline 和 exporter | v1.0 已完成 | 消除 ~50% parse_meta 调用 |
| `BufWriter` 默认 8KB | `BufWriter::with_capacity(16MB)` | v1.0 已完成 | 显著减少系统调用频率 |

**Phase 4 的优化空间集中在未经过 micro-benchmark 隔离验证的部分：** 格式化层内部是否还有可优化的细节，需要 `bench_csv_format_only` 量化后才能确定。

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | criterion 0.7 + `cargo test` (rust built-in) |
| Config file | `Cargo.toml` `[[bench]]` + `[dev-dependencies]` |
| Quick run command | `cargo test --lib -- exporter::csv 2>&1 \| tail -5` |
| Full suite command | `cargo test` |
| Benchmark quick run | `CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0 csv_export/10000` |
| Benchmark full run | `CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PERF-02 | real-file 吞吐相比 v1.0 baseline 提升 ≥10% | benchmark（criterion） | `CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0 csv_export_real/real_file` | ✅ bench_csv.rs（含 real-file group） |
| PERF-03 | 格式化路径吞吐提升（criterion micro-benchmark） | benchmark（criterion，新增） | `cargo bench --bench bench_csv -- csv_format_only` | ❌ Wave 0 需新增 `bench_csv_format_only` group |
| PERF-08 | 热循环内堆分配显著减少（flamegraph 对比可见） | 手工验证（flamegraph diff） | `samply record ... cargo bench --profile flamegraph ...` | ✅（samply 已安装，Phase 3 已用） |
| 无回归 | 629+ tests 全部通过 | unit/integration | `cargo test` | ✅ |
| 格式正确性 | CSV 输出格式无变化 | unit（csv.rs 内测试） | `cargo test --lib -- exporter::csv` | ✅（现有 12 个测试） |

### Sampling Rate

- **每次提交前：** `cargo test --lib -- exporter::csv` （快速格式化正确性验证）
- **每个 Wave 完成：** `cargo test` + `CRITERION_HOME=benches/baselines cargo bench --bench bench_csv -- --baseline v1.0`
- **Phase gate（进入 `/gsd-verify-work` 前）：** 全量 `cargo test` 绿色 + criterion 对比报告显示 ≥10% 提升

### Wave 0 Gaps

- [ ] `benches/bench_csv.rs` 新增 `bench_csv_format_only` group — 覆盖 PERF-03（格式化路径隔离）
- [ ] `write_record_preparsed` 需改为 `pub(crate)` 或通过公开路径可 benchmark（当前为私有 `fn`）
- [ ] `docs/flamegraphs/csv_export_real_phase4.json` — Phase 4 flamegraph（与 Phase 3 对比）

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo bench` | criterion benchmarks | ✓ | Rust 1.85（Edition 2024）| — |
| `samply` | flamegraph 采集 | ✓ | Phase 3 已用，符号可读 | — |
| `sqllogs/` 真实日志目录 | `bench_csv_real_file` | ? | 538MB 2 文件（Phase 3 时存在）| bench 自动 skip（CI-safe）|

---

## Open Questions

1. **格式化层净开销是多少？**
   - What we know：synthetic benchmark 含解析 + 格式化总开销 ~2.127ms/10k；flamegraph 显示 parse_meta 是 top-1
   - What's unclear：纯格式化（不含 parse_meta/parse_pm）占总开销的比例
   - Recommendation：Wave 0 添加 `bench_csv_format_only`，用预解析的记录测格式化路径，量化后决定是否值得优化

2. **line_buf 的实际初始容量（运行时自适应后）是多少？**
   - What we know：初始化为 2048 字节；clear() 保留容量；reserve 按实际 sql_len 动态申请
   - What's unclear：真实日志的 SQL 平均长度（影响 reserve 是否真的触发 realloc）
   - Recommendation：可在第一条记录后打印 `line_buf.capacity()`，若容量稳定在某值则 reserve 几乎从不触发扩容

3. **`find_indicators_split()` 的调用频次是否可减少？**
   - What we know：`parse_performance_metrics()` 内部调用一次 `find_indicators_split()`；`parse_meta()` 不调用此函数
   - What's unclear：是否可以在 `Sqllog` 的返回值中缓存 `split` 位置（上游 crate 不可修改）
   - Recommendation：Phase 4 范围内不可修改上游 crate；如 Phase 6 确认 dm-database-parser-sqllog 1.0.0 有新 API 则评估

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | 16MB vs 1MB BufWriter 对单线程顺序写入吞吐影响微乎其微 | Standard Stack Alternatives | 若错误，减小缓冲区可能带来负面影响；需 benchmark 验证 |
| A2 | `reserve()` 在已有足够容量时接近零成本（单次 capacity 检查） | Common Pitfalls | 若不同 Rust 版本有不同实现，优化效果可能不同 |
| A3 | `_platform_memmove` 主要来自解析层的字节拷贝，而非格式化层的 extend_from_slice | Hot Path Analysis | 若错误，格式化层可能还有更多优化空间 |

---

## Sources

### Primary (HIGH confidence)

- 直接代码审计：`src/exporter/csv.rs`、`src/cli/run.rs`、`src/features/replace_parameters.rs` [VERIFIED: 源文件直接读取]
- 上游 crate 源码：`~/.cargo/registry/.../dm-database-parser-sqllog-1.0.0/src/sqllog.rs` [VERIFIED: 直接读取]
- Phase 3 BENCHMARKS.md 实测数值 [VERIFIED: benches/BENCHMARKS.md]
- Phase 3 baseline JSON 数值（csv_export/10000 median = 2,127,322 ns）[VERIFIED: benches/baselines/csv_export/10000/v1.0/estimates.json]
- Phase 3 SUMMARY（Top 3 热路径：parse_meta、LogIterator::next、_platform_memmove）[VERIFIED: .planning/phases/03-profiling-benchmarking/03-03-SUMMARY.md]
- Cargo.toml 依赖列表 [VERIFIED: Cargo.toml]

### Secondary (MEDIUM confidence)

- Rust Vec::reserve() 零成本（容量足够时）：来自 Rust 标准库文档和 Rust Book [CITED: doc.rust-lang.org]
- itoa vs format! 性能对比（3-5x）：来自 itoa crate README [CITED: crates.io/crates/itoa]

### Tertiary (LOW confidence)

- `write_vectored` 对 BufWriter 后 CPU 开销无显著收益 [ASSUMED]
- flamegraph 符号质量（Phase 3 人工核验通过，但具体 %占比未定量记录）[ASSUMED]

---

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — 全部来自 Cargo.toml 直接读取
- Architecture: HIGH — 全部来自源码直接分析
- Pitfalls: MEDIUM — 部分来自通用 Rust 性能优化知识，非本项目专属验证
- Hot path findings: HIGH — 直接读取源码 + Phase 3 flamegraph 数据

**Research date:** 2026-04-27
**Valid until:** 2026-05-27（稳定 Rust 项目，30 天内无需重新研究）

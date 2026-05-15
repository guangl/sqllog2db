# Phase 10: 热路径优化 - Context

**Gathered:** 2026-05-14
**Status:** Ready for planning

<domain>
## Phase Boundary

在 FILTER-03（exclude 过滤器 OR-veto 语义）和 PERF-11（双重 regex 编译消除，CLI 启动 ≈ 3ms）就位后，用 samply + criterion 重新 profile 热路径：若 samply 显示 src/ 下某单一函数占 self time >5% 且有明确优化空间，则实施优化并提供 criterion 数据佐证；否则在 BENCHMARKS.md 记录"已达当前瓶颈"结论。

**不在范围内：** 新过滤功能、新 CLI 命令、配置格式变更、Nyquist 补签（Phase 11）。

</domain>

<decisions>
## Implementation Decisions

### Profiling 工具链

- **D-P1:** 使用 `samply`（无需 sudo，macOS Instruments 后端采样，已安装于 `~/.cargo/bin/samply`）。不使用 `cargo-flamegraph`（macOS 上依赖 dtrace，SIP 开启时采样不准）。
- **D-P2:** 跑两个 profiling 场景：
  1. `samply record ./target/release/sqllog2db run -c config.toml`（真实日志，覆盖完整热路径含过滤器）
  2. 针对 samply 指出的可疑热点，用 criterion bench 做微基准定量验证
- **D-P3:** samply 输出在浏览器中查看，以截图 + 文字描述（top N 函数及 self time 占比）的形式写入 `benches/BENCHMARKS.md` Phase 10 节，不保存二进制 profile 文件。

### Benchmark 场景扩展

- **D-B1:** 在 `benches/bench_filters.rs` 补充两个 exclude 过滤器场景：
  - `exclude_passthrough`：exclude 配置存在但无记录命中（测量纯过滤开销）
  - `exclude_active`：有记录被 OR-veto 排除（测量实际排除路径开销）
- **D-B2:** 用单字段 `exclude_username` 代表全局——所有 exclude 字段内部路径等价，单字段足以量化开销。
- **D-B3:** `exclude_passthrough` 场景：合成日志 username 固定为 `"BENCH"`，exclude 配置为 `["BENCH_EXCLUDE"]`，保证零命中。`exclude_active` 场景：exclude 配置为 `["BENCH"]`，所有记录均被排除（极端压力场景）。

### 热点门控判定标准

- **D-G1:** 门控标准：samply 中某单一函数占全局 self time **>5%**，且该函数属于 `src/` 下 sqllog2db 自身业务逻辑（非第三方库内部），且存在明确减少分配/clone/循环的优化路径——满足全部三条则实施优化。
- **D-G2:** 第三方库内部开销（`regex` crate 内部、`alloc`、`memchr` 等）即使占比 >5% 也**不**算可消除热点，不触发优化。
- **D-G3:** 若无符合条件的 >5% 热点，在 `benches/BENCHMARKS.md` Phase 10 节用简短段落文字记录"已达当前瓶颈"结论，附 top N 函数列表与 criterion 对比数据，**不**需要单独创建 VERIFICATION.md。

### 预设优化方向

- **D-O1:** 完全数据驱动——等 samply 告知实际热点再决定优化方向，不预设具体候选。planner 需为"有热点"和"无热点"两个分支各写计划。
- **D-O2:** 若实施优化，必须提供 criterion 优化前/后同场景 throughput 数据作为佳效依据，写入 BENCHMARKS.md Phase 10 节。
- **D-O3:** 优化涉及修改 `src/` 时，质量闸：`cargo clippy --all-targets -- -D warnings`、`cargo test`（全量）、criterion 新 baseline 不低于优化前（throughput 无回归）。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 需求与路线图
- `.planning/REQUIREMENTS.md` — PERF-10 完整描述（flamegraph 门控、>5% 热点判定）
- `.planning/ROADMAP.md` §Phase 10 — Success Criteria（3 条验收标准）

### 前序阶段上下文
- `.planning/phases/08-exclude-filter/08-CONTEXT.md` — FILTER-03 exclude 过滤器实现决策（OR-veto 语义、7 个字段）
- `.planning/phases/09-cli/09-CONTEXT.md` — PERF-11 双重编译消除决策、hyperfine 测量方案

### 关键实现文件
- `benches/bench_filters.rs` — 现有 filter benchmark，需补充 exclude_passthrough / exclude_active 场景
- `benches/BENCHMARKS.md` — Phase 10 profiling 结论写入此文件
- `src/features/filters.rs` — FilterProcessor 热路径（should_keep、exclude OR-veto 逻辑）
- `src/cli/run.rs` — 主运行循环，pipeline.is_empty() 快路径所在

### Profiling 工具
- `~/.cargo/bin/samply` — 已安装，使用 `samply record ./target/release/sqllog2db run -c config.toml`
- `Cargo.toml` §[profile.flamegraph] — release + debug=true，供 profiling 构建使用

</canonical_refs>

<code_context>
## Existing Code Insights

### 热路径结构
- `src/cli/run.rs` 主循环：`pipeline.is_empty()` 快路径已就位；过滤路径调用 `FilterProcessor::process()`
- `src/features/filters.rs` `FilterProcessor::process()`：包含 include 过滤（`CompiledMetaFilters`）+ Phase 8 新增 exclude OR-veto 逻辑
- `benches/bench_filters.rs`：现有 5 个场景（no_pipeline/pipeline_passthrough/trxid_small/trxid_large/indicator_prescan），无 exclude 场景

### Profiling 构建配置
- `Cargo.toml` `[profile.flamegraph]`：`inherits = "release"`, `debug = true`, `strip = "none"`
- 用 `cargo build --profile flamegraph` 构建后再运行 samply

### Integration Points
- 新 exclude benchmark 场景需复用 `bench_filters.rs` 现有的 `base_toml()`、`synthetic_log()` 辅助函数，添加 `cfg_exclude_passthrough()` 和 `cfg_exclude_active()` 配置函数

</code_context>

<specifics>
## Specific Ideas

- 两个场景都跑：先 samply run 看全局热点，再用 criterion 对可疑函数做微基准定量——顺序很重要，不要直接跳到 criterion
- samply 结论以"截图 + top N 文字描述"记录，无需保存二进制文件
- exclude_active 场景中所有记录均被排除（100% hit rate），是 OR-veto 逻辑的极端压力场景

</specifics>

<deferred>
## Deferred Ideas

None — 讨论全程在 Phase 10 范围内。

</deferred>

---

*Phase: 10-热路径优化*
*Context gathered: 2026-05-14*

# Phase 10: 热路径优化 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-14
**Phase:** 10-热路径优化
**Areas discussed:** Profiling 工具链, Benchmark 场景扩展, 热点门控判定标准, 预设优化方向

---

## Profiling 工具链

| Option | Description | Selected |
|--------|-------------|----------|
| samply（推荐） | 已安装，`samply record` 即可，无需 sudo，macOS Instruments 后端采样 | ✓ |
| cargo flamegraph | 已安装，需 sudo，SIP 开启时 dtrace 采样延迟高不准 | |
| 仅用 criterion 数据 | 不运行内核 profiler，纯靠 throughput 比较判定热点 | |
| 两者都跑 | samply 主要，cargo flamegraph 交叉对比 | |

**User's choice:** samply（推荐）

### Profiling 目标场景

| Option | Description | Selected |
|--------|-------------|----------|
| run 命令 + 真实 log（推荐） | `samply record ./target/release/sqllog2db run -c config.toml`，包含完整过滤路径 | |
| criterion bench 内嵌 samply | 精确到 benchmark 函数层级，设置较复杂 | |
| 两个场景都跑 | 先 run 看全局，再 bench 针对可疑热点验证 | ✓ |

**User's choice:** 两个场景都跑

### samply 结果保存

| Option | Description | Selected |
|--------|-------------|----------|
| 截图 + 文字描述写入报告（推荐） | 运行 samply 后手动记录 top N 函数及占比 | ✓ |
| 导出 profraw 文件 | samply 不直接导出，需额外工具转换 | |
| 仅在 CI 中运行 | CI 无法运行内核 profiler，不实际 | |

**User's choice:** 截图 + 文字描述写入报告（推荐）

### 报告写入位置

| Option | Description | Selected |
|--------|-------------|----------|
| benches/BENCHMARKS.md 新建 Phase 10 节（推荐） | 与 Phase 9 CLI 基线保持一致，单一文件记录所有性能数据 | ✓ |
| 单独新建 PROFILING.md | 与 BENCHMARKS.md 分离，增加文件数量 | |
| phases/10-*/10-VERIFICATION.md | 直接写入验收报告，不单独建基准文件 | |

**User's choice:** benches/BENCHMARKS.md 新建 Phase 10 节（推荐）

---

## Benchmark 场景扩展

### 是否补充 exclude 场景

| Option | Description | Selected |
|--------|-------------|----------|
| 补充 exclude 场景（推荐） | 新增 exclude_passthrough 和 exclude_active，使 profile 基础反映完整过滤器组合 | ✓ |
| 不补充 | 现有 benchmark 无 exclude，但 samply 全局 profile 仍能看到实际热点 | |
| 仅加 exclude_active | 只加最具代表性的场景，减少场景数量 | |

**User's choice:** 补充 exclude 场景（推荐）

### 覆盖字段

| Option | Description | Selected |
|--------|-------------|----------|
| 单字段（username）代表全局（推荐） | 用 exclude_username 代表其他字段，内部路径等价 | ✓ |
| 多字段组合 | 同时开启 exclude_username + exclude_client_ip，测量 OR-veto 多字段开销 | |
| 所有 7 个字段全开启 | 最大压力场景，但场景设置复杂，收益较小 | |

**User's choice:** 单字段（username）代表全局（推荐）

### exclude_passthrough 合成日志 username

| Option | Description | Selected |
|--------|-------------|----------|
| username="BENCH"，exclude="BENCH_EXCLUDE"（推荐） | 合成日志 username 均为 BENCH，exclude 配置 BENCH_EXCLUDE，保证零命中 | ✓ |
| username="NO_MATCH" 等任意不存在字符串 | 更简洁，直接用绝对不匹配的字符串 | |
| 你决定 | planner 按令人满意的合成方式设置 | |

**User's choice:** username="BENCH"，exclude="BENCH_EXCLUDE"（推荐）

---

## 热点门控判定标准

### >5% 量化方式

| Option | Description | Selected |
|--------|-------------|----------|
| samply 单一函数 >5% stack time（推荐） | samply 显示某函数占全局 self time >5%，视觉上直接 | ✓ |
| criterion throughput 回归 >5% | 加载新 benchmark 场景后 criterion 显示 >5% throughput 下降 | |
| 两者同时满足（AND 语义） | samply AND criterion 都指向同一热点才优化，防止误判 | |
| 主观判断 | 看到明显优化点即实施，不设确切数字门控 | |

**User's choice:** samply 单一函数 >5% stack time（推荐）

### "可消除"定义

| Option | Description | Selected |
|--------|-------------|----------|
| 属于我们自己的业务逻辑（推荐） | 热点属于 sqllog2db 自身代码（src/）且有明确优化空间 | ✓ |
| 不包含第三方库内部开销 | regex crate 内部、alloc 等必要开销不算入热点，即使占比 >5% | |
| 你决定 | planner 按运行时实际 profile 结果判断 | |

**User's choice:** 属于我们自己的业务逻辑（推荐）

### 无热点时结论格式

| Option | Description | Selected |
|--------|-------------|----------|
| BENCHMARKS.md 简单段论文字（推荐） | Phase 10 节写"已达当前瓶颈"+ top N 函数列表 + criterion 对比数据 | ✓ |
| VERIFICATION.md 正式签署 | 类似 Phase 9 那样建一个验收报告，明确标识"已达当前瓶颈" | |
| 两个都写 | BENCHMARKS.md 记录数据，VERIFICATION.md 签署结论 | |

**User's choice:** BENCHMARKS.md 简单段论文字（推荐）

---

## 预设优化方向

### 是否预设优化候选

| Option | Description | Selected |
|--------|-------------|----------|
| 完全数据驱动（推荐） | 等 samply 告知实际热点再决定优化方向，planner 需为两个分支写计划 | ✓ |
| 有具体候选预设 | 已有已知可能的优化点，如 should_keep 内联、exclude 路径分配减少等 | |

**User's choice:** 完全数据驱动（推荐）

### 优化佳效依据

| Option | Description | Selected |
|--------|-------------|----------|
| criterion 数据对比（推荐） | 优化前/后同场景 criterion throughput，写入 BENCHMARKS.md Phase 10 节 | ✓ |
| samply 前/后对比 | 优化前后各跑一次 samply，对比函数占比变化 | |
| 两者各提一个 | samply 看热点是否消失，criterion 看 throughput 提升多少 | |

**User's choice:** criterion 数据对比（推荐）

### 质量闸

**User's choice:** cargo clippy（--all-targets -- -D warnings）+ cargo test（全量）+ criterion 新 baseline 不低于优化前

---

## Claude's Discretion

无——所有关键决策用户均已明确选择。

## Deferred Ideas

None — 讨论全程在 Phase 10 范围内。

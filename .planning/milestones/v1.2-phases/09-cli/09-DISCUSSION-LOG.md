# Phase 9: CLI 启动提速 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-11
**Phase:** 9-CLI 启动提速
**Areas discussed:** validate_and_compile() 接口, update check 处置, hyperfine 测量范围

---

## validate_and_compile() 接口

| Option | Description | Selected |
|--------|-------------|----------|
| compile() 直接返回 Result | 废弃 validate_regexes()，让 compile_patterns() 返回 ConfigError；validate 与 run 路径各调用一次 compile() | ✓ |
| 新增 validate_and_compile() | 保留现有函数签名不变，额外新增 FiltersFeature::validate_and_compile() → Result<CompiledMetaFilters> | |
| 你来决定 | 让 researcher/planner 选择最合适方案 | |

**User's choice:** compile() 直接返回 Result

**后续问题：**

| validate 路径如何处理 CompiledMetaFilters | Selected |
|---|---|
| 直接丢弃（validate 只检查错误，不保存结果） | ✓ |
| 传递给 run 路径 | |

| .expect("regex validated") 处理 | Selected |
|---|---|
| ? 传播错误（try_from_meta 返回 Result） | ✓ |
| 保留 expect（validate 先运行确保安全） | |

| validate_regexes() 保留与否 | Selected |
|---|---|
| 完全删除 | ✓ |
| 保留但内部调用 compile | |

---

## update check 处置

| Option | Description | Selected |
|--------|-------------|----------|
| 无条件后台化 | std::thread::spawn fire-and-forget，主流程不阻塞 | ✓ |
| 先量化再决定 | hyperfine 测出占比后若 >50ms 才后台化 | |
| 完全移除 | startup 不做 update check | |

**User's choice:** 无条件后台化

**后续问题：**

| 时序/交错处理 | Selected |
|---|---|
| 接受交错，不处理（日志走 stderr） | ✓ |
| 主流程结束前 join | |
| 你来决定 | |

---

## hyperfine 测量范围

| 测量命令 | Selected |
|---|---|
| --version + validate 两个命令 | ✓ |
| 只测 validate | |
| 只测 --version | |

| 报告对比维度 | Selected |
|---|---|
| 优化前 vs 优化后 | ✓ |
| 有 regex 配置 vs 无 regex | ✓ |
| 仅记录优化后 | ✓（最低要求） |

| 数据保存方式 | Selected |
|---|---|
| 写入 benches/BENCHMARKS.md | ✓ |
| 写入 VERIFICATION.md | |
| 仅 commit 说明 | |

| 未达预期阈值处理 | Selected |
|---|---|
| 记录数据即可，无需达到阈值 | ✓ |
| 设定具体阈值，测不到则预警 | |

---

## Claude's Discretion

无——所有决策点均由用户明确选择，无"你来决定"项。

## Deferred Ideas

- 具体启动时间目标值（如 <100ms）——未来里程碑可参考 Phase 9 数据设定

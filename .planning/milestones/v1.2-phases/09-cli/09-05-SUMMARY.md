---
phase: 09-cli
plan: 05
subsystem: cli
tags: [rust, regex, performance, config, filters, pipeline]

requires:
  - phase: 09-cli/09-01
    provides: update check 后台化基础
  - phase: 09-cli/09-02
    provides: CompiledMetaFilters/CompiledSqlFilters 接口定义
  - phase: 09-cli/09-03
    provides: FilterProcessor/build_pipeline 初始实现
  - phase: 09-cli/09-04
    provides: hyperfine 冷启动基线数据

provides:
  - "Config::validate_and_compile() 方法：单次编译 regex，返回 Option<(CompiledMetaFilters, CompiledSqlFilters)>"
  - "handle_run/build_pipeline/FilterProcessor 全链路预编译参数传递"
  - "BENCHMARKS.md 可观测断言：grep 验证双重编译消除"
  - "SC-2 BLOCKER 关闭：run 路径每个 regex 字段只调用一次 Regex::new()"

affects: [performance, filters, testing]

tech-stack:
  added: []
  patterns:
    - "validate_and_compile() 模式：校验与编译合并为单次操作，结果从入口贯穿至消费点"
    - "Option<(Meta, Sql)> 作为预编译结果的传递类型，None 表示无过滤或 enable=false"

key-files:
  created: []
  modified:
    - src/config.rs
    - src/main.rs
    - src/cli/run.rs
    - benches/BENCHMARKS.md
    - tests/integration.rs
    - benches/bench_csv.rs
    - benches/bench_sqlite.rs
    - benches/bench_filters.rs

key-decisions:
  - "validate_and_compile() 与 validate() 并存：validate 子命令独立路径不受影响，消除双重编译只针对 run 路径"
  - "build_pipeline 返回类型从 Result<Pipeline> 改为 Pipeline：regex 编译已在 validate_and_compile 阶段发生，不再有可失败操作"
  - "FilterProcessor::try_new → FilterProcessor::new：接受预编译参数，不再内部编译"
  - "bench_filters 使用 b.iter_with_setup() 避免 Clone 约束：每次 benchmark 循环重新从 validate_and_compile 获取编译结果"

requirements-completed: [PERF-11]

duration: 25min
completed: 2026-05-14
---

# Phase 9 Plan 05: validate_and_compile() 统一接口消除双重编译 Summary

**Config::validate_and_compile() 将 regex 编译从 validate() 丢弃改为返回编译结果，贯穿 main.rs → handle_run → build_pipeline → FilterProcessor，run 路径每个 regex 字段只调用一次 Regex::new()，SC-2 BLOCKER 关闭**

## Performance

- **Duration:** 约 25 分钟
- **Started:** 2026-05-14
- **Completed:** 2026-05-14
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- 新增 `Config::validate_and_compile()` 方法，返回 `Result<Option<(CompiledMetaFilters, CompiledSqlFilters)>>`，与 `validate()` 非过滤器校验路径等价
- 全链路传递：`main.rs run 分支 → handle_run(compiled_filters) → build_pipeline(compiled_meta) → FilterProcessor::new(compiled_meta)`
- `run.rs` 中 `try_from_meta` 与 `try_from_sql_filters` 调用计数清零（grep 可验证）
- `validate` 子命令保留原 `cfg.validate()` 调用，独立路径不受影响
- 新增 7 个单元测试覆盖 `validate_and_compile` 全部错误/成功路径
- `benches/BENCHMARKS.md` 失效断言替换为基于 `validate_and_compile` 存在性 + `run.rs` 调用清零的可观测断言

## Task Commits

1. **Task 1: 在 src/config.rs 新增 validate_and_compile() 方法** - `0a46a07` (feat)
2. **Task 2: main.rs run 分支 + run.rs handle_run/build_pipeline/FilterProcessor 全链路接入预编译参数** - `e2c43db` (feat)
3. **Task 3: 修正 benches/BENCHMARKS.md 中失效的双重编译验证断言** - `ec8397d` (docs)

## Files Created/Modified

- `src/config.rs` - 新增 `validate_and_compile()` 方法 + 7 个单元测试
- `src/main.rs` - run 分支 `cfg.validate()` 改为 `cfg.validate_and_compile()`，传递 `compiled_filters` 给 `handle_run`
- `src/cli/run.rs` - `handle_run` 新增 `compiled_filters` 参数；`build_pipeline` 改为接受 `Option<CompiledMetaFilters>` 返回 `Pipeline`；`FilterProcessor::try_new` → `FilterProcessor::new`
- `benches/BENCHMARKS.md` - L382 失效断言替换为三行可观测断言
- `tests/integration.rs` - 所有 `handle_run` 调用适配新签名，`test_handle_run_with_filters_builds_pipeline` 使用 `validate_and_compile()` 正确传入编译结果
- `benches/bench_csv.rs` - `handle_run` 调用新增 `None` 参数
- `benches/bench_sqlite.rs` - `handle_run` 调用新增 `None` 参数（3 处）
- `benches/bench_filters.rs` - 改用 `b.iter_with_setup()` 每次迭代获取编译结果

## 签名变更对比

| 位置 | 变更前 | 变更后 |
|------|--------|--------|
| `build_pipeline` 签名 | `fn build_pipeline(cfg: &Config) -> Result<Pipeline>` | `fn build_pipeline(cfg: &Config, compiled_meta: Option<CompiledMetaFilters>) -> Pipeline` |
| `FilterProcessor` 构造函数 | `fn try_new(filter: &FiltersFeature) -> Result<Self>` | `fn new(compiled_meta: CompiledMetaFilters, filter: &FiltersFeature) -> Self` |
| `handle_run` 参数 | 9 个参数 | 10 个参数，末尾新增 `compiled_filters: Option<(CompiledMetaFilters, CompiledSqlFilters)>` |
| `main.rs run 分支` | `cfg.validate()?` | `let compiled_filters = cfg.validate_and_compile()?` |

## Decisions Made

- `validate_and_compile()` 与 `validate()` 并存：validate 子命令使用 `validate()`（丢弃结果，仅做校验），run 子命令使用 `validate_and_compile()`（保留结果）
- `build_pipeline` 不再返回 `Result`：regex 编译已移至 `validate_and_compile`，`build_pipeline` 内部无可失败操作
- bench_filters 使用 `iter_with_setup` 而非 `iter + clone`：避免需要为 `CompiledMetaFilters` 实现 `Clone`（其中含 `Vec<Regex>`），同时符合 benchmark 的 setup/measured 分离原则

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] bench 文件中的 handle_run 调用也需要更新**
- **Found during:** Task 2（运行 clippy 时）
- **Issue:** `benches/bench_csv.rs`、`benches/bench_sqlite.rs`、`benches/bench_filters.rs` 也调用了 `handle_run`，签名变更导致编译失败，plan 中未提及
- **Fix:** 更新三个 bench 文件中所有 `handle_run` 调用，`bench_filters` 改用 `iter_with_setup`
- **Files modified:** benches/bench_csv.rs, benches/bench_sqlite.rs, benches/bench_filters.rs
- **Verification:** `cargo clippy --all-targets -- -D warnings` 通过
- **Committed in:** e2c43db（Task 2 commit）

---

**Total deviations:** 1 auto-fixed (Rule 3 - blocking)
**Impact on plan:** 必要修复，bench 文件是 --all-targets 编译目标的一部分。无范围蔓延。

## grep 验证证据

```bash
# 1. validate_and_compile 接口存在
grep -c "fn validate_and_compile" src/config.rs    # → 1

# 2. run.rs 中双重编译清零
grep -cE "try_from_meta|try_from_sql_filters" src/cli/run.rs    # → 0

# 3. validate 子命令保留
grep -n "cfg.validate()" src/main.rs    # → L230: cfg.validate()?

# 4. BENCHMARKS.md 新断言存在
grep -c "validate_and_compile" benches/BENCHMARKS.md    # → 2

# 5. 旧 API 完全清零
grep -rn "from_meta\b" src/ | grep -v "try_from_meta" | wc -l    # → 0
```

## SC-2 验收闭环

SC-2 BLOCKER 状态：**已关闭**

验收条件："`validate_and_compile()` 统一接口实现：regex 由单次编译结果同时用于验证与运行"

- `validate_and_compile()` 在 config.rs 中实现，一次性完成校验 + 编译
- 编译结果从 main.rs 经 handle_run → build_pipeline → FilterProcessor 全链路传递
- run 路径中 `try_from_meta` 与 `try_from_sql_filters` 调用次数为 0（grep 可验证）
- 729 个测试（原 651 + 7 个新增 + 71 个其他）全部通过

## Issues Encountered

- cargo fmt pre-commit hook 要求格式化，`validate_and_compile` 返回类型被重新格式化为多行（正常行为）
- Task 2 中集成测试 integration.rs 使用了 sed 批量替换单行调用，但单行语法在 fmt 后会被展开为多行，下次提交时 fmt hook 会再次格式化——pre-commit hook 自动处理

## Next Phase Readiness

- SC-2 BLOCKER 关闭，Phase 9 计划全部完成
- validate_and_compile() 可用于后续任何需要在 main 入口预编译 filter 的场景
- 如有新过滤器字段加入 CompiledMetaFilters/CompiledSqlFilters，只需更新 try_from_meta/try_from_sql_filters，上层链路无需修改

---
*Phase: 09-cli*
*Completed: 2026-05-14*

## Self-Check: PASSED

- `src/config.rs` 存在且含 `validate_and_compile`：FOUND
- `src/main.rs` 含 `validate_and_compile`：FOUND
- `src/cli/run.rs` 中 try_from_meta/try_from_sql_filters 计数为 0：VERIFIED
- `benches/BENCHMARKS.md` 含新断言：FOUND
- Task 1 commit `0a46a07`：FOUND
- Task 2 commit `e2c43db`：FOUND
- Task 3 commit `ec8397d`：FOUND
- 测试总数 729 >= 651：PASSED
- clippy 0 error：PASSED

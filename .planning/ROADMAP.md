# Roadmap: sqllog2db

## Milestones

- ✅ **v1.0 增强 SQL 内容过滤与字段投影** — Phases 1–2 (shipped 2026-04-18)
- ✅ **v1.1 性能优化** — Phases 3–6 (shipped 2026-05-10)
- 🚧 **v1.2 质量强化 & 性能深化** — Phases 7–11 (in progress)

## Phases

<details>
<summary>✅ v1.0 增强 SQL 内容过滤与字段投影 (Phases 1–2) — SHIPPED 2026-04-18</summary>

- [x] Phase 1: 正则字段过滤 (2/2 plans) — completed 2026-04-18
- [x] Phase 2: 输出字段控制 (4/4 plans) — completed 2026-04-18

Full details: `.planning/milestones/v1.0-ROADMAP.md`

</details>

<details>
<summary>✅ v1.1 性能优化 (Phases 3–6) — SHIPPED 2026-05-10</summary>

- [x] Phase 3: Profiling & Benchmarking (3/3 plans) — completed 2026-04-27
- [x] Phase 4: CSV 性能优化 (4/4 plans) — completed 2026-05-09
- [x] Phase 5: SQLite 性能优化 (3/3 plans) — completed 2026-05-10
- [x] Phase 6: 解析库集成 + 验收 (2/2 plans) — completed 2026-05-10

Full details: `.planning/milestones/v1.1-ROADMAP.md`

</details>

### 🚧 v1.2 质量强化 & 性能深化 (In Progress)

**Milestone Goal:** 消灭已知技术债，补全过滤缺口，进一步提升解析/过滤热路径与 CLI 启动速度。

- [ ] **Phase 7: 技术债修复** - 修复 SQLite 静默错误与 SQL 注入风险
- [ ] **Phase 8: 排除过滤器** - 实现 FILTER-03 排除模式（7 个元数据字段 exclude_* 支持）
- [x] **Phase 9: CLI 启动提速** - 消除双重 regex 编译，后台化 update check (5/5 plans) — completed 2026-05-14
- [ ] **Phase 10: 热路径优化** - flamegraph 门控的热路径深化优化
- [ ] **Phase 11: Nyquist 补签** - 补全 Phase 3/4/5/6 VALIDATION.md compliant 签署

## Phase Details

### Phase 7: 技术债修复
**Goal**: SQLite 导出错误路径可观测、table_name 配置安全可靠
**Depends on**: Nothing (no code dependencies)
**Requirements**: DEBT-01, DEBT-02
**Success Criteria** (what must be TRUE):
  1. 用户在 SQLite 初始化时遭遇"表不存在"等无害错误时，工具正常继续运行且不打印误导性输出
  2. 用户遭遇真实 SQLite 错误时，错误信息写入配置的 error log 文件而非被静默丢弃
  3. 用户配置非法 `table_name`（含特殊字符）时，`cargo run -- validate` 报错并拒绝启动
  4. 用户配置合法 `table_name` 时，DROP/DELETE/CREATE/INSERT 四条 DDL 均使用双引号转义，SQL 注入向量消除
**Plans**: 1 plan
- [x] 07-01-PLAN.md — DEBT-01 SQLite 静默错误显式化 + DEBT-02 table_name ASCII 校验与 DDL 双引号转义

### Phase 8: 排除过滤器
**Goal**: 用户可通过配置"匹配则丢弃"规则精确排除不需要的记录，与现有包含过滤形成完整的 AND/OR-veto 语义
**Depends on**: Phase 7
**Requirements**: FILTER-03
**Success Criteria** (what must be TRUE):
  1. 用户在 config 中配置任意 `exclude_*` 字段后，匹配该字段的记录从输出中消失
  2. 七个元数据字段（username/client_ip/sess_id/thrd_id/statement/appname/tag）均可独立配置排除规则
  3. 排除规则为 OR veto 语义：任意一个 exclude 字段命中即丢弃该记录
  4. 未配置任何 `exclude_*` 字段时，`pipeline.is_empty()` 快路径不受影响，零额外开销
  5. 非法排除正则在 `cargo run -- validate` 阶段报错，不推迟到运行时 panic
**Plans**: 2 plans

**Wave 1**
- [x] 08-01-PLAN.md — filters.rs 核心：MetaFilters/CompiledMetaFilters 扩展、OR-veto should_keep、has_any_filters、validate_regexes、单元测试

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 08-02-PLAN.md — run.rs has_any_filters() 预计算 + init.rs 配置模板 exclude_* 注释

**Cross-cutting constraints:**
- `cargo clippy --all-targets -- -D warnings` 必须在每个 plan 后通过（两个 plan 均要求）
- `cargo test` 全量通过（两个 plan 均要求）

### Phase 9: CLI 启动提速
**Goal**: CLI 冷启动时间可量化且双重 regex 编译消除，用 hyperfine 数据作为门控
**Depends on**: Phase 8
**Requirements**: PERF-11
**Success Criteria** (what must be TRUE):
  1. `hyperfine` 基线测量完成并记录在验收报告中，冷启动时间有明确数字
  2. `validate_and_compile()` 统一接口实现：regex 由单次编译结果同时用于验证与运行，不存在双重 Regex::new() 调用
  3. 若 update check 在基线中占比 >50ms，则移入后台线程，主流程不阻塞
  4. 全部 651 测试通过，无回归
**Plans**: 5 plans

**Wave 1** *(并行执行)*
- [x] 09-01-PLAN.md — filters.rs 核心重构：compile_patterns 新签名、try_from_meta、try_from_sql_filters、删除 validate_regexes 系列
- [x] 09-02-PLAN.md — update check 后台化：check_for_updates_at_startup 改为 thread::spawn fire-and-forget

**Wave 2** *(blocked on Wave 1 / 09-01)*
- [x] 09-03-PLAN.md — config.rs + run.rs 接入：validate() 调用 try_from_meta，FilterProcessor::try_new，build_pipeline 返回 Result

**Wave 3** *(blocked on Wave 2)*
- [x] 09-04-PLAN.md — hyperfine 基线测量 + benches/BENCHMARKS.md "Phase 9 CLI 冷启动基线" 节记录

**Wave 4** *(gap closure — blocked on Wave 3 / 09-04; closes VERIFICATION.md SC-2 BLOCKER)*
- [x] 09-05-PLAN.md — validate_and_compile() 统一接口 + 全链路传参，彻底消除 run 路径双重 regex 编译；修正 BENCHMARKS.md 失效断言

**Cross-cutting constraints:**
- `cargo clippy --all-targets -- -D warnings` 必须在每个 plan 后通过
- `cargo test` 全量通过（651 个测试）

### Phase 10: 热路径优化
**Goal**: 在 FILTER-03 与 PERF-11 就位后，用 samply + criterion 量化剩余热点并按 D-G1 门控决策是否优化
**Depends on**: Phase 9
**Requirements**: PERF-10
**Success Criteria** (what must be TRUE):
  1. criterion + samply 重新 profile 完成，报告反映包含排除过滤器后的真实热路径形态
  2. 若 samply 显示 >5% 可消除热点（src/ 业务逻辑 + 明确优化路径），则优化实施并有 criterion 数据佐证效果
  3. 若无符合条件的热点，则 BENCHMARKS.md Phase 10 节记录"已达当前瓶颈"结论并签署，不做无依据的优化
  4. 全部 651 测试通过，基准无回归（≤5% 容差）
**Plans**: 3 plans（Wave 1 始终执行；Wave 2 二选一互斥执行）

**Wave 1** *(始终执行)*
- [x] 10-01-PLAN.md — exclude bench 场景补全（exclude_passthrough / exclude_active）+ samply profiling + BENCHMARKS.md Phase 10 节 D-G1 门控判定签署

**Wave 2** *(blocked on Wave 1；二选一互斥，由 10-01 §D-G1 门控判定 文本决定)*
- [ ] 10-02-PLAN.md — Branch B-yes：若 10-01 判定"命中 D-G1"，实施 samply 指认的热点函数优化 + criterion 前/后吞吐对比
- [ ] 10-03-PLAN.md — Branch B-no：若 10-01 判定"未命中 D-G1"，BENCHMARKS.md 增补 §当前瓶颈分析 + "已达当前瓶颈"签署

**Cross-cutting constraints:**
- `cargo clippy --all-targets -- -D warnings` 必须在每个 plan 后通过
- `cargo test` 全量通过（≥651 测试）
- criterion 任一 bench 场景回归不得 > 5%（D-O3 容差）

### Phase 11: Nyquist 补签
**Goal**: Phase 3/4/5/6 的 VALIDATION.md compliant 状态全部补签完整，Nyquist 审计链无缺口
**Depends on**: Nothing (pure documentation)
**Requirements**: DEBT-03
**Success Criteria** (what must be TRUE):
  1. Phase 3 VALIDATION.md 包含完整的 Nyquist compliant 签署条目
  2. Phase 4 VALIDATION.md 包含完整的 Nyquist compliant 签署条目
  3. Phase 5 VALIDATION.md 包含完整的 Nyquist compliant 签署条目
  4. Phase 6 VALIDATION.md 包含完整的 Nyquist compliant 签署条目
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. 正则字段过滤 | v1.0 | 2/2 | Complete | 2026-04-18 |
| 2. 输出字段控制 | v1.0 | 4/4 | Complete | 2026-04-18 |
| 3. Profiling & Benchmarking | v1.1 | 3/3 | Complete | 2026-04-27 |
| 4. CSV 性能优化 | v1.1 | 4/4 | Complete | 2026-05-09 |
| 5. SQLite 性能优化 | v1.1 | 3/3 | Complete | 2026-05-10 |
| 6. 解析库集成 + 验收 | v1.1 | 2/2 | Complete | 2026-05-10 |
| 7. 技术债修复 | v1.2 | 0/1 | Not started | - |
| 8. 排除过滤器 | v1.2 | 0/2 | Not started | - |
| 9. CLI 启动提速 | v1.2 | 5/5 | Complete | 2026-05-14 |
| 10. 热路径优化 | v1.2 | 1/3 | In Progress|  |
| 11. Nyquist 补签 | v1.2 | 0/TBD | Not started | - |

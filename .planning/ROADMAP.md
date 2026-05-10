# Roadmap: sqllog2db

## Milestones

- ✅ **v1.0 增强 SQL 内容过滤与字段投影** — Phases 1–2 (shipped 2026-04-18)
- 🔄 **v1.1 性能优化** — Phases 3–6 (active)

## Phases

<details>
<summary>✅ v1.0 增强 SQL 内容过滤与字段投影 (Phases 1–2) — SHIPPED 2026-04-18</summary>

- [x] Phase 1: 正则字段过滤 (2/2 plans) — completed 2026-04-18
- [x] Phase 2: 输出字段控制 (4/4 plans) — completed 2026-04-18

Full details: `.planning/milestones/v1.0-ROADMAP.md`

</details>

### v1.1 性能优化

- [x] **Phase 3: Profiling & Benchmarking** — 建立性能基准，定位热路径瓶颈（completed 2026-04-27）
- [ ] **Phase 4: CSV 性能优化** — 提升 CSV 导出吞吐量，减少热循环分配
- [ ] **Phase 5: SQLite 性能优化** — 批量事务、prepared statement 复用（WAL 模式已移除，数据无需崩溃保护）
- [ ] **Phase 6: 解析库集成 + 验收** — 集成 dm-database-parser-sqllog 1.0.0 新 API，回归验证

## Phase Details

### Phase 3: Profiling & Benchmarking
**Goal**: 开发者能够量化 CSV 和 SQLite 的实际瓶颈位置，建立可复现的性能基准，作为后续优化的决策依据
**Depends on**: Nothing (first phase of v1.1)
**Requirements**: PERF-01
**Success Criteria** (what must be TRUE):
  1. criterion benchmark 能对 CSV 和 SQLite 导出路径分别输出 records/sec 吞吐数值，并可在 CI 环境重复运行
  2. flamegraph 生成成功，能指出热路径中占比最高的函数调用链（如格式化、写入、SQL 编译等）
  3. 基准报告记录 v1.0 的当前吞吐基准（CSV real-file ~1.55M records/sec），作为 Phase 4/5 优化目标的参照
**Plans**: 3 plans
- [x] 03-01-PLAN.md — Cargo.toml 新增 [profile.flamegraph] + bench_csv real-file group
- [x] 03-02-PLAN.md — bench_sqlite real-file group
- [x] 03-03-PLAN.md — 采集 v1.0 baseline、生成 flamegraph、更新 BENCHMARKS.md（Wave 2，含 human-verify checkpoint）

### Phase 4: CSV 性能优化
**Goal**: CSV 导出在 real 1.1GB 日志文件上吞吐可量化提升 ≥10%，热循环堆分配显著减少
**Depends on**: Phase 3
**Requirements**: PERF-02, PERF-03, PERF-08
**Success Criteria** (what must be TRUE):
  1. real 1.1GB 文件的 CSV 导出速度相比 Phase 3 基准提升 ≥10%（criterion 或计时脚本可验证）
  2. 热循环内消除至少一处可识别的隐藏 clone 或不必要的堆分配（通过 flamegraph 对比 Phase 3 可见差异）
  3. CSV 格式化路径在 criterion micro-benchmark 中吞吐提升（与 Phase 3 baseline 对比输出 diff 报告）
  4. 629+ 测试全部通过，无功能退化
**Plans**: TBD

### Phase 5: SQLite 性能优化
**Goal**: SQLite 导出速度显著提升，单行提交开销被批量事务消除，prepared statement 得到复用
**Depends on**: Phase 3
**Requirements**: PERF-04, PERF-06
**Success Criteria** (what must be TRUE):
  1. SQLite 导出使用显式事务分组（如每 N 条提交一次），criterion benchmark 显示相比单行提交速度提升可量化
  2. prepared statement 在写入循环中只编译一次，通过代码审查确认无重复 `prepare()` 调用
  3. 50+ 测试全部通过，无功能退化
  4. ~~WAL 模式~~ — 用户决策移除：数据无需崩溃保护，保留 `JOURNAL_MODE=OFF SYNCHRONOUS=OFF` 高性能模式
**Plans**: 3 plans

**Wave 1**
- [x] 05-01-PLAN.md — config.rs 新增 batch_size 字段 + bench_sqlite.rs 单行提交对照 group

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 05-02-PLAN.md — sqlite.rs 批量事务 + prepare_cached 复用（WAL 代码已回退）

**Wave 3** *(blocked on Wave 1+2 completion)*
- [x] 05-03-PLAN.md — criterion benchmark 运行 + BENCHMARKS.md Phase 5 数值更新（含 human-verify checkpoint）

**Cross-cutting constraints:**
- `cargo clippy --all-targets -- -D warnings` 零警告（所有 Wave 结束后）
- `cargo test` 50+ 测试全部通过（所有 Wave 结束后）
- 函数体不超过 40 行（initialize() 提取 initialize_pragmas() 辅助函数）

### Phase 6: 解析库集成 + 验收
**Goal**: dm-database-parser-sqllog 1.0.0 新 API 已评估并按需集成，所有 629+ 测试通过，v1.1 milestone 可交付
**Depends on**: Phase 4, Phase 5
**Requirements**: PERF-07, PERF-09
**Success Criteria** (what must be TRUE):
  1. PERF-07 调研结论有明确记录：若新 API 存在零拷贝或批量解析接口则集成，若无则记录原因并关闭
  2. 所有 629+ 现有测试在最终代码上全部通过（`cargo test` 输出 0 failures）
  3. `cargo clippy --all-targets -- -D warnings` 无警告，`cargo fmt` 无 diff
**Plans**: 2 plans
- [ ] 06-01-PLAN.md — 提交 Phase 4/5 遗留变更 + 记录 PERF-07 调研结论
- [ ] 06-02-PLAN.md — 全量验收（cargo test + clippy + fmt），更新 ROADMAP

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. 正则字段过滤 | v1.0 | 2/2 | Complete | 2026-04-18 |
| 2. 输出字段控制 | v1.0 | 4/4 | Complete | 2026-04-18 |
| 3. Profiling & Benchmarking | v1.1 | 3/3 | Complete | 2026-04-27 |
| 4. CSV 性能优化 | v1.1 | 0/? | Not started | — |
| 5. SQLite 性能优化 | v1.1 | 0/3 | Not started | — |
| 6. 解析库集成 + 验收 | v1.1 | 0/2 | Not started | — |

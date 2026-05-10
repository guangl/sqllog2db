# Retrospective

## Milestone: v1.0 — 增强 SQL 内容过滤与字段投影

**Shipped:** 2026-04-18
**Phases:** 2 | **Plans:** 6

### What Was Built

- Pre-compiled regex filter structs (`CompiledMetaFilters` + `CompiledSqlFilters`) with AND cross-field / OR intra-field semantics, startup validation
- `FilterProcessor` hot path integrated with compiled regex on all 7 meta fields
- `FeaturesConfig::ordered_field_indices()` for user-specified field order projection
- `CsvExporter` + `SqliteExporter` extended with `ordered_indices` — full field projection pipeline
- End-to-end wiring through `ExporterManager` and parallel CSV path

### What Worked

- **TDD RED/GREEN pattern** — writing failing tests first caught interface design issues early (Plan 01-01)
- **Pre-compile at startup** strategy — moving regex compilation to startup (not hot loop) kept the performance guarantee simple to reason about
- **`#[allow(dead_code)]` staging** — marking new structs as dead_code in Plan 01, removing in Plan 02 made the two-plan dependency explicit and clean
- **Atomic plan commits** — each plan had a clean, reviewable commit; deviations (clippy fixes) were folded in without scope creep

### What Was Inefficient

- REQUIREMENTS.md checkboxes were never updated during phase execution — required manual acknowledgement at milestone close
- STATE.md Performance Metrics section was left with placeholder dashes throughout the milestone (not auto-populated)

### Patterns Established

- `ordered_indices: Vec<usize>` as the projection API — cleaner than FieldMask bitmask for arbitrary ordering
- Reference-based construction (`FilterProcessor::new(&FiltersFeature)`) avoids clippy `needless_pass_by_value` from the start
- Re-export compiled types via `features::mod` for a clean public API boundary

### Key Lessons

- Clippy `-D warnings` catches interface design issues (pass-by-value, dead_code, must_use) that are cheaper to fix during the plan than after
- Two-plan structure (core structs → hot path wiring) worked well for regex feature: Plan 01 was pure logic, Plan 02 was pure integration — no mixing
- `ordered_indices` replacing FieldMask was the right call: the FieldMask approach would have required separate ordering metadata anyway

### Cost Observations

- Sessions: single-day execution (2026-04-18)
- Notable: all 6 plans executed sequentially in one session with no context resets required

---

## Milestone: v1.1 — 性能优化

**Shipped:** 2026-05-10
**Phases:** 4 | **Plans:** 12

### What Was Built

- Flamegraph + criterion benchmark 基础设施（CSV/SQLite 双路径，real-file + synthetic）
- CSV 条件 reserve + `include_performance_metrics` 配置项，热循环分配减少
- SQLite `batch_commit_if_needed()` 批量事务（5x 性能差距）+ `prepare_cached()` statement 复用
- dm-database-parser-sqllog 1.0.0 升级 + PERF-07 API 评估存档
- 651 测试全部通过，clippy 零警告

### What Worked

- **Profile-first approach** — Phase 3 先用 flamegraph 定位热路径，避免了在 CSV 格式化层（只占 ~5%）投入过多时间；真正的热点（parse_meta/memmove）在上游 crate，Phase 6 通过升级自动获益
- **accept-defer 机制** — PERF-02 real-file 数据无法在 agent 环境采集，用户明确 accept-defer，避免了阻塞整个 milestone
- **用户决策快速关闭** — WAL 模式（PERF-05）实测超 hard limit，用户当场决策移除，ROADMAP 即时更新，无返工
- **Wave 设计** — Phase 5 三波次（config → 实现 → benchmark）解耦得干净，每波次可独立验证
- **parallel csv + sqlite paths** — Phase 4 和 Phase 5 并行规划，无依赖冲突，节省时间

### What Was Inefficient

- **Nyquist VALIDATION.md 停留在 draft** — 4 个 phase 的 VALIDATION.md 均未更新为 compliant，作为文档债务带入 v1.2
- **06-02-PLAN.md Task 2/3 未执行** — ROADMAP 和 REQUIREMENTS 状态更新被 orchestrator 跳过，需要 Phase 6 验收后人工确认（实际在 VERIFICATION.md 的 Human Verification 中处理）
- **SUMMARY frontmatter requirements-completed 缺字段** — Phase 6 两个 SUMMARY 缺少该字段，3-source 交叉验证只有 2/3 sources

### Patterns Established

- `batch_commit_if_needed()` 模式 — 每 N 行提交一次，`row_count % batch_size == 0` 判断，简单有效
- `prepare_cached()` 替代 `prepare()` — rusqlite StatementCache LRU，对所有 export 路径统一应用
- CI-safe benchmark skip — `if !real_dir.exists() { eprintln!(...); return; }` 模式，保证 CI 不 panic
- Phase-level accept-defer — 在 VERIFICATION.md frontmatter 记录 override 和 accepted_by，形成审计轨迹

### Key Lessons

- 性能优化前必须 profile：假设 CSV 格式化是瓶颈是错的，真正的热路径在上游 crate
- hard limit = median × 1.05 是一个好的容差设计，既有弹性又有约束力
- WAL 模式不一定比 journal_mode=OFF 快——在写入密集场景下，WAL 的 checkpointing 开销可能反而更高
- accept-defer 要在 VERIFICATION.md 中留有迹可查，否则后续 audit 会困惑

### Cost Observations

- Sessions: 多 session 执行（2026-04-26 → 2026-05-10，14 天）
- Notable: Phase 5 WAL 实现→回退产生了额外工作（feat→fix），但用户决策快速，总体无阻塞

---

## Cross-Milestone Trends

| Metric | v1.0 | v1.1 |
|--------|------|------|
| Phases | 2 | 4 |
| Plans | 6 | 12 |
| Days | 1 | 14 |
| Auto-fixed deviations | 6 (all clippy) | 1 (WAL revert) |
| Scope creep | 0 | 0 |
| Test suite at close | 629+ | 651 |
| Accept-defer decisions | 0 | 1 (PERF-02 real-file) |
| User scope removals | 0 | 1 (PERF-05 WAL) |

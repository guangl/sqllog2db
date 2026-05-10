# Phase 6: 解析库集成 + 验收 - Context

**Gathered:** 2026-05-10
**Status:** Ready for planning

<domain>
## Phase Boundary

评估 dm-database-parser-sqllog 1.0.0 新 API 并记录调研结论；将 Cargo.toml 升级至 1.0.0；清理 Phase 4/5 未提交遗留变更；最终验收 v1.1 milestone（629+ 测试通过、clippy 零警告、fmt 无 diff）。

</domain>

<decisions>
## Implementation Decisions

### PERF-07 结论（解析库新 API 调研）

- **D-01:** PERF-07**关闭**，理由：mmap 零拷贝和 `par_iter()` 在 0.9.1 已存在，当前预扫描路径（`scan_log_file_for_matches`）已调用 `parser.par_iter()`，自动获得两级并行。
- **D-02:** 1.0.0 改进（更完整的编码检测：头+尾双采样；`MADV_SEQUENTIAL` 预读 hint；小文件 par_iter 单分区优化）对现有代码**自动生效**，无需代码变更。
- **D-03:** 新增的 `index()` / `RecordIndex` API（两阶段字节偏移索引扫描）不适用于当前流式写入场景，不集成。记录原因后关闭 PERF-07。
- **D-04:** Phase 6 的代码改动仅限 `Cargo.toml` 版本升级（0.9.1 → 1.0.0，已完成），`cargo check` 确认无 API 破坏性变更。

### 验收标准

- **D-05:** 验收仅需以下三项，不跑 criterion benchmark：
  1. `cargo test` — 629+ 测试全部通过，0 failures
  2. `cargo clippy --all-targets -- -D warnings` — 零警告
  3. `cargo fmt --check` — 无 diff

### 未提交变更处理

- **D-06:** 以下 Phase 4/5 遗留的未提交变更统一纳入 Phase 6 提交：
  - `Cargo.toml` / `Cargo.lock` — 1.0.0 升级（Phase 6 核心变更）
  - `config.toml` — Phase 5 简化遗留
  - `benches/baselines/sqlite_export/*/change/estimates.json` — Phase 5 benchmark 对比产物
  - `.planning/phases/04-csv/04-REVIEW.md` + `04-REVIEW-FIX.md` — Phase 4 review 补丁
  - `benches/baselines/sqlite_single_row/`、`sqlite_export_real/real_file/base/`、`sqlite_export/*/base/` — Phase 5 新基线

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### 解析库版本与 API
- `Cargo.toml` — 当前依赖版本；确认 `dm-database-parser-sqllog = "1.0.0"` 已写入
- `~/.cargo/registry/src/.../dm-database-parser-sqllog-1.0.0/src/lib.rs` — 1.0.0 公开 API（`LogParser`, `LogIterator`, `RecordIndex`, `parse_record`）
- `~/.cargo/registry/src/.../dm-database-parser-sqllog-1.0.0/src/parser.rs` — `par_iter()`、`index()` 实现细节

### 热路径集成点
- `src/cli/run.rs` L287–330 — `scan_log_file_for_matches`：已调用 `parser.par_iter()`，1.0.0 改进自动生效
- `src/cli/run.rs` L432–560 — `process_csv_parallel`：每文件独立 `parser.iter()`（单文件顺序，跨文件并行）
- `src/cli/run.rs` L140–260 — `process_log_file`：顺序路径（SQLite 写入，非 Send）

### 验收基准
- `benches/BENCHMARKS.md` — Phase 3/4/5 性能数值记录（本阶段不跑 benchmark，仅供参照）
- `.planning/REQUIREMENTS.md` — PERF-07（解析库新特性）、PERF-09（测试回归）

### Phase 4/5 遗留
- `.planning/phases/04-csv/04-REVIEW.md` — Phase 4 code review；`04-REVIEW-FIX.md` 为修复记录

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `parser.par_iter()` — 已在预扫描中使用；1.0.0 小文件单分区优化自动生效
- `LogParser::from_path()` — 所有文件处理入口，mmap + encoding 自动检测

### Established Patterns
- 两级并行：跨文件 `log_files.par_iter()` + 文件内 `parser.par_iter()`（预扫描）
- 单线程顺序：SQLite 写入路径（`rusqlite::Connection` 非 Send）
- 每文件独立 writer + 合并：并行 CSV 路径（`process_csv_parallel`）

### Integration Points
- `Cargo.toml` — 唯一需要变更的文件（版本已更新，`cargo check` 通过）
- 未提交变更需在 Phase 6 验收前统一提交，保证 `git status` 干净

</code_context>

<specifics>
## Specific Ideas

- PERF-07 调研结论需写入文档（可在 plan 阶段以 commit message 或 BENCHMARKS.md 注释记录）
- `cargo check` 已通过，Phase 6 执行时直接跑 `cargo test` 即可，无需调试编译错误

</specifics>

<deferred>
## Deferred Ideas

- `index()` / `RecordIndex` 两阶段并行扫描 — 不适用当前流式写入场景，可在未来有大规模并行需求时重新评估
- criterion benchmark 对比验证 1.0.0 升级无退化 — 理论无风险，如有疑虑可在 CI 中补充

</deferred>

---

*Phase: 6-parser-acceptance*
*Context gathered: 2026-05-10*

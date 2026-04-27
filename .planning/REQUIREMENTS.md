# Requirements: sqllog2db v1.1 性能优化

**Milestone:** v1.1 性能优化
**Created:** 2026-04-26
**Status:** Active

---

## v1.1 Requirements

### Profiling & Measurement

- [x] **PERF-01**: 开发者能够通过 criterion benchmark 和 flamegraph 定位 CSV 和 SQLite 导出的热路径瓶颈，并生成可复现的基准报告

### CSV 性能

- [ ] **PERF-02**: CSV 导出吞吐在 real 1.1GB 日志文件上相比 v1.0 基准（~1.55M records/sec）有可量化提升（目标 ≥10%）
- [ ] **PERF-03**: CSV 格式化/序列化路径优化（减少字符串分配、改进 buffer 策略或利用更快的格式化 API）

### SQLite 性能

- [ ] **PERF-04**: SQLite 导出使用批量事务（batch INSERT 或显式 transaction 分组），减少单行提交开销
- [ ] **PERF-05**: SQLite 导出启用 WAL 模式，提升并发写入与读写分离性能
- [ ] **PERF-06**: SQLite prepared statement 复用——避免每行重新编译 SQL

### 解析库新特性

- [ ] **PERF-07**: 调研 dm-database-parser-sqllog 1.0.0 新 API，若存在零拷贝或批量解析接口则集成到主路径

### 内存/CPU

- [ ] **PERF-08**: 热循环内减少堆分配（SmallVec / compact_str 等已有 crate 的充分利用，或消除隐藏 clone）
- [ ] **PERF-09**: 所有优化完成后，现有 629+ 测试套件全部通过，无功能退化

---

## Future Requirements

- 异步 I/O（tokio）导出路径 — 复杂度高，当前单线程流式已满足需求
- 多线程 SQLite 写入 — WAL 模式优先验证，多写线程冲突处理留后续
- JSONL 导出性能 — v1.1 专注 CSV + SQLite

## Out of Scope

- 改变 CLI 接口或配置格式 — v1.1 仅内部优化，对用户透明
- 引入新的导出格式 — 不在此 milestone
- 修改过滤逻辑行为 — 已在 v1.0 完成

---

## Traceability

| REQ-ID | Phase | Status |
|--------|-------|--------|
| PERF-01 | Phase 3 | complete (2026-04-27) |
| PERF-02 | Phase 4 | pending |
| PERF-03 | Phase 4 | pending |
| PERF-04 | Phase 5 | pending |
| PERF-05 | Phase 5 | pending |
| PERF-06 | Phase 5 | pending |
| PERF-07 | Phase 6 | pending |
| PERF-08 | Phase 4 | pending |
| PERF-09 | Phase 6 | pending |

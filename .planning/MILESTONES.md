# Milestones

## v1.1 — 性能优化

**Shipped:** 2026-05-10
**Phases:** 3–6 | **Plans:** 12 | **Commits:** ~85

### Delivered

通过 profiling 定位热路径后，系统性提升 CSV 和 SQLite 导出性能，升级上游解析库至 1.0.0，651 测试全部通过。

### Key Accomplishments

1. Flamegraph + criterion benchmark 基础设施——定位 `parse_meta`/`LogIterator::next`/`_platform_memmove` 为主热路径（PERF-01）
2. CSV 条件 reserve + `bench_csv_format_only` 格式化层量化（~19.7M elem/s）；合成 benchmark 改善 -8.53%（PERF-02/03/08）
3. `include_performance_metrics=false` 配置项——完全跳过 `parse_performance_metrics()` 和 memrchr 扫描，热循环堆分配显著减少（PERF-08）
4. SQLite 批量事务 `batch_commit_if_needed()`——5x 性能差距（35.4ms vs 7.1ms/10k rows），WAL 模式用户决策移除（PERF-04）
5. SQLite `prepare_cached()`——三条导出路径复用已编译 statement，消除每行 `sqlite3_prepare_v3()` 开销（PERF-06）
6. dm-database-parser-sqllog 1.0.0 升级——mmap/par_iter/MADV_SEQUENTIAL 自动生效；index() API 不集成（流式无收益）；651 测试通过（PERF-07/09）

### Stats

- Rust LOC: ~9,889 (src/)
- Files modified: 165 (+11,746 / -5,638 lines)
- Test suite: 651 tests passing
- Timeline: 14 days (2026-04-26 → 2026-05-10)
- Performance: SQLite 5x batch improvement; CSV synthetic -8.53%; ~5.2M records/sec CSV synthetic

### Archive

- `.planning/milestones/v1.1-ROADMAP.md`
- `.planning/milestones/v1.1-REQUIREMENTS.md`

---

## v1.0 — 增强 SQL 内容过滤与字段投影

**Shipped:** 2026-04-18
**Phases:** 1–2 | **Plans:** 6 | **Commits:** ~33

### Delivered

让用户能够精确指定"导出哪些记录的哪些字段"——正则过滤 + AND 语义 + 输出字段控制全部上线。

### Key Accomplishments

1. Pre-compiled regex filter structs (`CompiledMetaFilters` + `CompiledSqlFilters`) with AND cross-field / OR intra-field semantics and startup validation via `Config::validate()`
2. `FilterProcessor` hot path integrated with compiled regex — regex filters on all 7 meta fields (usernames, client_ips, sess_ids, thrd_ids, statements, appnames, tags)
3. `FeaturesConfig::ordered_field_indices()` — returns user-specified field order as `Vec<usize>` for downstream projection
4. `CsvExporter` extended with `ordered_indices` — `build_header()` and `write_record_preparsed()` now project fields in user-specified order
5. `SqliteExporter` extended with `ordered_indices` — `build_create_sql()`, `build_insert_sql()`, `do_insert_preparsed()` all project by ordered index
6. End-to-end wiring through `ExporterManager::from_config()` and parallel CSV path `process_csv_parallel()`

### Stats

- Rust LOC: ~9,889
- Files modified: ~10 source files
- Test suite: 629+ tests passing
- Performance: zero-overhead fast path preserved (pipeline.is_empty() unchanged)

### Archive

- `.planning/milestones/v1.0-ROADMAP.md`
- `.planning/milestones/v1.0-REQUIREMENTS.md`

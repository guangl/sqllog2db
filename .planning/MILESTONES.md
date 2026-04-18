# Milestones

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

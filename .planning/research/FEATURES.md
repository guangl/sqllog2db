# Feature Research

**Domain:** CLI SQL log analysis tool — SQL template normalization, workload statistics, SVG chart generation
**Researched:** 2026-05-15
**Milestone:** v1.3
**Confidence:** HIGH (existing codebase analysis) / MEDIUM (external tool patterns)

---

## Context: What Already Exists

Before categorizing new features, the relevant existing capabilities that v1.3 builds on:

- `sql_fingerprint::fingerprint()` — literals replaced with `?`, whitespace collapsed (ALREADY DONE)
- `cli/digest.rs` — `DigestEntry` with count, total/avg/max exec_ms, example_sql, first_seen (ALREADY DONE)
- `cli/stats.rs` — time-bucket aggregation, group-by user/app/ip, slow query heap (ALREADY DONE)
- `features/replace_parameters.rs` — parameter substitution into SQL placeholders (ALREADY DONE)
- `Pipeline` + `LogProcessor` trait — pluggable processor architecture (ALREADY DONE)
- `FeaturesConfig` in `config.rs` — TOML deserialization + validation pattern (ALREADY DONE)

The `digest` command already produces per-fingerprint statistics (count, avg/max exec). The v1.3 work
**extends** this foundation rather than replacing it:
1. Richer template normalization (comment stripping, IN list collapse, keyword casing)
2. Richer statistics (p50/p95/p99 percentiles + histogram buckets)
3. Persistent output (JSON/CSV report files, SQLite table, CSV companion)
4. Visual output (SVG charts)

---

## Table Stakes

Features users expect from any SQL workload analysis tool. Missing = incomplete product.

| Feature | Why Expected | Complexity | Existing Dependency |
|---------|--------------|------------|---------------------|
| **TMPL-01**: Template normalization via config | Every SQL digest tool (pt-query-digest, MySQL Performance Schema, pg_stat_statements) groups queries by normalized template; users discovering `digest` command expect this to be configurable | MEDIUM | Extends `fingerprint()` in `sql_fingerprint.rs`; output feeds `DigestEntry` |
| **TMPL-01**: Comment stripping | SQL logs from ORMs (Hibernate, MyBatis) often embed `/* comment */` hints that break grouping | LOW | New pass in normalization pipeline; pure string processing |
| **TMPL-01**: IN list collapse | `IN (1,2,3)` vs `IN (1,2,3,4,5)` should map to same template; pt-query-digest does this | LOW | Regex or state-machine replacement; confined to normalization module |
| **TMPL-01**: Keyword uppercase normalization | Templates should compare consistently; lowercase SQL should normalize the same as uppercase | LOW | Simple `to_uppercase()` pass on keyword tokens OR full lowercasing |
| **TMPL-02**: p50/p95/p99 per template | Averages mask tail latency; p95/p99 are standard DBA metrics for SLA analysis | MEDIUM | Requires storing all per-template samples OR approximate algorithm (see Pitfalls) |
| **TMPL-02**: min/max alongside percentiles | Industry standard: pt-query-digest reports min/max/avg/95pct for every metric | LOW | Already have total/max; need to track min |
| **TMPL-03**: Standalone JSON report output | Machine-readable output is required for CI pipelines, alerting, and programmatic consumption | LOW | `serde_json` already in Cargo.toml; pattern identical to `DigestJson` in `digest.rs` |
| **TMPL-03**: Standalone CSV report output | DBAs use CSV for spreadsheet analysis; simpler to consume than JSON for non-programmers | LOW | `itoa`/`ryu` already used; write with `BufWriter` |
| **TMPL-04**: SQLite `sql_templates` table | Users using SQLite export expect template data alongside raw records in the same database | MEDIUM | `rusqlite` already in Cargo.toml; needs DDL + batch INSERT |
| **TMPL-04**: CSV `*_templates.csv` companion | Users using CSV export expect a parallel summary file without switching to SQLite | LOW | Reuses CSV writer infrastructure |
| **CHART-01**: Config-driven SVG output directory | Charts only generated when user explicitly enables them; no surprises in default mode | LOW | New config section `[features.template_stats.charts]` |
| **CHART-02**: Top-N template frequency bar chart | Answering "which queries run most often?" is the primary use case of every SQL digest tool | MEDIUM | Bar chart with template fingerprint labels on Y-axis |
| **CHART-03**: Execution time distribution histogram | Answering "how is query latency distributed?" requires a histogram, not just avg/max | MEDIUM | Logarithmic buckets (powers of 10, matching pt-query-digest convention) |

---

## Differentiators

Features that distinguish this tool from generic SQL digest tools.

| Feature | Value Proposition | Complexity | Existing Dependency |
|---------|-------------------|------------|---------------------|
| **TMPL-02**: Histogram bucket data in JSON output | pt-query-digest only shows histogram visually; exporting bucket counts as data enables downstream tooling | LOW | Bucket array added to JSON output struct |
| **TMPL-02**: Per-template histogram (not just global) | pt-query-digest shows a global histogram; per-template histograms identify bimodal distributions in individual queries | MEDIUM | Memory cost: 10-20 buckets per template entry in `HashMap` |
| **CHART-04**: Execution frequency time trend line chart | Shows when SQL load spiked; directly answers "was this query always slow or did it start recently?" | MEDIUM | Depends on `stats.rs` time-bucket data already collected |
| **CHART-05**: User/Schema activity pie chart | Immediately identifies which users or schemas dominate workload; useful for capacity planning | MEDIUM | Depends on `stats.rs` group-by data already collected |
| **TMPL-01**: Normalization rules configurable per-run | Users can tune normalization aggressiveness (e.g., disable IN-list collapse for debugging) | LOW | Boolean flags in `TemplateNormalizationConfig` |
| **TMPL-03**: `first_seen` / `last_seen` timestamps in report | Helps identify recently-introduced slow queries; `digest.rs` already captures `first_seen` | LOW | Add `last_seen` field to `FingerprintAccumulator` |

---

## Anti-Features

Features to explicitly avoid.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Interactive HTML dashboard** | Requires a web server or embedded JS runtime; breaks the "static file" contract; html+js is platform-dependent to open | Generate standalone SVG files that open in any browser without JS; clearly document this limitation |
| **Exact p50/p95/p99 via full sample storage** | Storing every `exec_time_ms` per template blows memory: 100K templates × 1M samples = hundreds of GB | Use fixed-size histogram buckets (10-20 logarithmic buckets cover 0.001ms to 1000s); document that percentiles are approximate |
| **Template diff across time windows** | Comparing "this week vs last week" requires two-pass storage and complex diff logic; out of scope for a streaming CLI | Users can run the tool on date-filtered log files separately and compare JSON outputs manually |
| **Real-time / live-tail mode** | Would require inotify/kqueue polling; fundamentally changes the tool's batch-processing model | Batch processing is the correct model for log files; document this explicitly |
| **SQL parsing / AST-based normalization** | A full SQL parser (sqlparser-rs, nom-sql) is a heavy dependency, adds compile time, and is DM-dialect-specific | Rule-based normalization (regex + string state machine) is sufficient for grouping purposes; exact semantic equivalence is not required |
| **Per-template alert thresholds** | Config becomes complex; threshold management is a monitoring system's job (Prometheus, Grafana) | Export JSON; let downstream monitoring tools handle alerting |
| **Template clustering / similarity detection** | "These two templates are 90% similar" requires edit-distance computation across all template pairs; O(N²) cost | Group by exact fingerprint match only; document that near-duplicates appear as separate entries |
| **Normalization applied to the main export columns** | Changing `normalized_sql` column behavior mid-stream breaks existing consumers | Template normalization lives in the statistics path (`template_stats` feature) separate from `replace_parameters` |

---

## Feature Dependencies

```
TMPL-01 (template normalization)
    └──extends──> sql_fingerprint::fingerprint() (existing)
    └──feeds──> TMPL-02 (statistics accumulator uses normalized template as key)
    └──independent of──> replace_parameters (different code path)

TMPL-02 (statistics accumulator — streaming)
    └──depends on──> TMPL-01 (normalized template as HashMap key)
    └──feeds──> TMPL-03, TMPL-04 (output of accumulated data)
    └──feeds──> CHART-02, CHART-03 (chart input is TMPL-02 data)
    └──note──> must NOT buffer all records in memory; histogram buckets are O(1) per template

TMPL-03 (standalone JSON/CSV report)
    └──depends on──> TMPL-02 (data source)
    └──independent of──> exporter choice (CSV or SQLite)
    └──writes──> user-specified output path from config

TMPL-04 (SQLite table / CSV companion)
    └──depends on──> TMPL-02 (data source)
    └──depends on──> exporter choice: SQLite → sql_templates table; CSV → *_templates.csv
    └──integrates with──> ExporterManager (flush hook after streaming completes)

CHART-01/02/03 (SVG charts)
    └──depends on──> TMPL-02 data (frequency bar, exec histogram)
    └──CHART-04 depends on──> stats.rs time-bucket data (already collected)
    └──CHART-05 depends on──> stats.rs group-by data (already collected)
    └──independent of──> main export path
    └──writes──> SVG files to config-specified directory
```

### Key Integration Point: Pipeline vs. Post-Processing

TMPL-02 (statistics accumulator) should be a **post-streaming accumulator**, not a `LogProcessor` in the Pipeline. Reason: the existing `Pipeline` contract returns `bool` (keep/discard record), but template statistics need to accumulate ALL records after filter decisions. The cleanest design:

1. Existing Pipeline runs filters (keep/discard)
2. After each kept record, a `TemplateStatsAccumulator` struct updates its `HashMap<String, TemplateEntry>`
3. After streaming completes, `TemplateStatsAccumulator::flush()` writes TMPL-03 reports and TMPL-04 tables

This avoids modifying the `LogProcessor` trait or `Pipeline` struct.

---

## Expected UX: Configuration Format

Consistent with existing `config.toml` style. New sections under `[features]`:

```toml
[features.template_stats]
# Enable SQL template statistics aggregation (default: false)
enable = false

# --- Normalization rules (all default to true when enable = true) ---
# Normalize SQL keywords to uppercase (SELECT → SELECT, select → SELECT)
normalize_keywords = true
# Strip -- and /* */ comments from SQL before fingerprinting
strip_comments = true
# Collapse IN (...) lists to IN (?) regardless of element count
collapse_in_lists = true

# --- Output: standalone report files ---
# Write per-template statistics to a JSON file (omit to skip)
# json_report = "reports/sql_templates.json"
# Write per-template statistics to a CSV file (omit to skip)
# csv_report = "reports/sql_templates.csv"

# --- Output: charts ---
# Directory to write SVG chart files (omit to skip chart generation)
# charts_dir = "reports/charts"
# Number of top templates to include in frequency bar chart (default: 20)
top_n = 20

# --- Filtering ---
# Minimum call count to include in report (default: 1)
min_count = 1
```

When `[exporter.sqlite]` is active AND `[features.template_stats]` is enabled, a `sql_templates` table
is automatically created in the same database. No additional config key is needed — presence of SQLite
exporter implies the companion table.

When `[exporter.csv]` is active AND `[features.template_stats]` is enabled, a companion `*_templates.csv`
file is written alongside the main CSV output (same directory, `_templates` suffix).

### SQLite `sql_templates` Table Schema (Recommended)

```sql
CREATE TABLE sql_templates (
    id              INTEGER PRIMARY KEY,
    fingerprint     TEXT NOT NULL UNIQUE,
    call_count      INTEGER NOT NULL,
    total_exec_ms   REAL NOT NULL,
    avg_exec_ms     REAL NOT NULL,
    min_exec_ms     REAL NOT NULL,
    max_exec_ms     REAL NOT NULL,
    p50_exec_ms     REAL,
    p95_exec_ms     REAL,
    p99_exec_ms     REAL,
    first_seen      TEXT,
    last_seen       TEXT,
    example_sql     TEXT
);
```

### JSON Report Schema (Recommended)

Mirrors `DigestJson` in `digest.rs` but adds percentile fields and histogram buckets:

```json
{
  "generated_at": "2026-05-15T10:00:00",
  "total_records": 1234567,
  "unique_templates": 42,
  "entries": [
    {
      "rank": 1,
      "fingerprint": "SELECT * FROM t WHERE id = ?",
      "call_count": 45230,
      "total_exec_ms": 90460.0,
      "avg_exec_ms": 2.0,
      "min_exec_ms": 0.1,
      "max_exec_ms": 350.0,
      "p50_exec_ms": 1.8,
      "p95_exec_ms": 12.0,
      "p99_exec_ms": 85.0,
      "first_seen": "2026-05-15 08:01:02",
      "last_seen": "2026-05-15 09:59:58",
      "example_sql": "SELECT * FROM t WHERE id = 12345",
      "histogram_buckets": [
        {"label": "<1ms", "count": 32000},
        {"label": "1-10ms", "count": 11000},
        {"label": "10-100ms", "count": 1800},
        {"label": ">100ms", "count": 430}
      ]
    }
  ]
}
```

---

## SVG Chart Design Principles

For a CLI tool generating static SVG files (no JS, no interactive library), the following principles
produce good output:

### What Makes a Good Minimal CLI SVG Chart

1. **Fully self-contained SVG**: All styles inline via `style="..."` attributes; no `<style>` blocks
   that might be stripped by SVG viewers. No external fonts (use `font-family="monospace"` or
   `sans-serif`).

2. **Fixed canvas size**: 800×400px covers most use cases. Large enough for labels, small enough to
   open quickly. Do not depend on dynamic layout.

3. **Bar chart (CHART-02)**: Horizontal bar chart (not vertical) for template labels — fingerprints
   are long strings, horizontal layout gives more readable space. Sort bars by frequency descending.
   Cap at `top_n` templates (default 20). Include count labels at bar end.

4. **Histogram (CHART-03)**: Logarithmic x-axis buckets matching pt-query-digest convention:
   `<0.001ms`, `0.001–0.01ms`, `0.01–0.1ms`, `0.1–1ms`, `1–10ms`, `10–100ms`, `>100ms`.
   Vertical bars. Label the p50/p95/p99 lines as vertical dashed overlays.

5. **Line chart (CHART-04)**: Time on x-axis (derived from time-bucket data in `stats.rs`),
   count on y-axis. Simple polyline with data point dots. Date labels at regular intervals.

6. **Pie chart (CHART-05)**: User/schema share. SVG `<path>` arc segments. Include a legend
   beside the pie (not embedded in the arc). Cap at top-10 segments + "Others" slice.

7. **Recommended library**: `plotters` with `default-features = false, features = ["svg"]`.
   The `plotters-svg` backend is a separate crate since plotters 0.3, enabling SVG-only builds
   without bitmap codec dependencies. Compile time is well under 10 seconds with this configuration.
   Alternative: generate raw SVG strings directly (zero dependencies, ~200 lines per chart type),
   which is viable given the small number of chart types.

---

## Complexity Notes

| Feature ID | Estimated Complexity | Rationale |
|------------|---------------------|-----------|
| TMPL-01 normalization rules | LOW-MEDIUM | String processing with memchr already available; IN-list collapse is the hardest part (stateful parser) |
| TMPL-02 histogram buckets | LOW | Fixed 7-10 log buckets; O(1) per record, O(templates) memory |
| TMPL-02 p50/p95/p99 from buckets | LOW | Linear interpolation within bucket; approximate but sufficient |
| TMPL-03 JSON output | LOW | Reuse `serde_json` pattern from `digest.rs` |
| TMPL-03 CSV output | LOW | Reuse `BufWriter` + `itoa`/`ryu` pattern from CSV exporter |
| TMPL-04 SQLite table | MEDIUM | Needs DDL + batch INSERT + integration with `ExporterManager` flush |
| TMPL-04 CSV companion | LOW | Write after main CSV is closed; same directory logic |
| CHART-01/02/03 bar + histogram | MEDIUM | plotters SVG backend; label layout for long fingerprints |
| CHART-04 time trend | MEDIUM | Depends on time-bucket data; x-axis label formatting |
| CHART-05 pie chart | MEDIUM | SVG arc math or plotters pie series |

---

## Sources

- 代码库阅读：`src/cli/digest.rs` — 现有 `DigestEntry`, `FingerprintAccumulator`, 指纹统计结构
- 代码库阅读：`src/cli/stats.rs` — 时间分桶、group-by 聚合，CHART-04/05 的数据来源
- 代码库阅读：`src/features/sql_fingerprint.rs` — 现有指纹化逻辑（字面量替换 + 空白折叠）
- 代码库阅读：`src/features/mod.rs` — `Pipeline` / `LogProcessor` 架构约束
- 代码库阅读：`Cargo.toml` — 已有依赖（`serde_json`, `rusqlite`, `itoa`, `ryu`, `memchr`）
- [pt-query-digest documentation](https://docs.percona.com/percona-toolkit/pt-query-digest.html) — 工业标准 SQL digest 工具的功能集（MEDIUM confidence，规范参考）
- [plotters-rs GitHub](https://github.com/plotters-rs/plotters) — Rust SVG 图表库，`default-features = false, features = ["svg"]` 可最小化二进制体积（MEDIUM confidence）
- [plotters-svg crate](https://github.com/plotters-rs/plotters-svg) — 独立 SVG backend crate（MEDIUM confidence）
- [charts-rs crate](https://docs.rs/crate/charts-rs/latest) — 备选图表库，支持 16 种图表类型，内置主题（LOW confidence，未深度验证）
- [pt-query-digest histogram convention](https://docs.percona.com/percona-toolkit/pt-query-digest.html) — 对数分桶（10µs → 1s 范围）是工业惯例（MEDIUM confidence）

---

*Feature research for: sqllog2db v1.3 — SQL 模板分析 & 可视化*
*Researched: 2026-05-15*

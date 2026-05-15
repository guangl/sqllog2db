# Technology Stack — v1.3 New Capabilities

**Project:** sqllog2db v1.3 SQL Template Analysis & SVG Charts
**Researched:** 2026-05-15
**Confidence:** HIGH (all primary findings verified via Context7 and official docs)

---

## Summary: What Needs to Be Added

The three new feature areas each need at most one new crate. Everything else can be
implemented in pure Rust using crates already in Cargo.toml (`regex`, `ahash`,
`memchr`, `serde_json`).

| Feature Area | New Crate | Version | Action |
|---|---|---|---|
| SQL template normalization | none (DIY extension) | — | Extend `sql_fingerprint.rs` |
| Percentile stats (p50/p95/p99 + buckets) | `hdrhistogram` | 7.5.4 | Add dependency |
| SVG chart generation | `plotters` | 0.3.7 | Add with SVG-only feature flags |

---

## 1. SQL Template Normalization

### Recommendation: DIY Extension of `sql_fingerprint.rs`

**Do NOT add `sqlparser` (0.62.0) or `sql-fingerprint` (1.11.1).**

**Why not `sqlparser`:**
- Full AST parser — compile overhead is substantial (used by DataFusion, Polars, etc.)
- DaMeng SQL dialect is not a supported dialect — `GenericDialect` will reject or
  mis-parse DaMeng-specific constructs (`ROWNUM`, DM-specific built-in functions)
- All four required normalizations (case unification, whitespace, comment removal, IN
  list unification) are simpler as byte-level passes identical in style to the existing
  `fingerprint()` function

**Why not `sql-fingerprint` (1.11.1):**
- Sole dependency: `sqlparser >= 0.62.0` with `visitor` feature — inherits all the
  parse overhead and DaMeng dialect problems above
- Its value-list reduction collapses column lists and alias lists too aggressively;
  `SELECT a, b FROM t` becomes `SELECT ... FROM t`, destroying template identity

**What to build — extend `src/features/sql_fingerprint.rs`:**

The existing `fingerprint()` function already handles the hardest parts: string-literal
skipping, whitespace folding, memchr-based bulk scanning. The four new transforms are
additive byte-walk branches:

| Transform | Implementation approach |
|---|---|
| Comment removal | Add `--` branch (scan forward to `\n`) and `/*` branch (scan forward to `*/`) in the main byte loop, before the existing `match` arms. Output nothing for comment content. |
| Keyword case unification | After comment/whitespace pass, uppercase token-start bytes in non-literal regions. A simple approach: collect contiguous `[A-Za-z_]` spans and `to_ascii_uppercase()` them. Or maintain a `const` keyword set (SELECT, FROM, WHERE, …) and case-fold only those tokens. |
| IN list unification | Detect `IN (` sequence (case-insensitive after keyword pass). When found, consume to the matching `)` while skipping nested string literals, and emit `IN (?)` instead. |
| Whitespace normalization | Already done — existing `fingerprint()` collapses runs to single space. |

**Integration:** The new function `normalize_template(sql: &str) -> String` becomes
the key for the `ahash::HashMap<String, TemplateStats>` aggregation map. It is called
once per record in the new pipeline processor; `ahash` is already in Cargo.toml.

**Confidence:** HIGH — existing code verified by reading `sql_fingerprint.rs` and
`replace_parameters.rs`; same byte-walk pattern used throughout.

---

## 2. Percentile Statistics (p50/p95/p99 + Histogram Buckets)

### Recommendation: `hdrhistogram` 7.5.4

```toml
[dependencies]
hdrhistogram = "7.5"
```

**API for this project:**

```rust
use hdrhistogram::Histogram;

// Per-template accumulation (streaming pass):
let mut hist = Histogram::<u64>::new(3)?;   // 3 significant figures
hist.record(exec_time_ms as u64)?;

// Post-collection queries (output phase):
let p50  = hist.value_at_percentile(50.0);
let p95  = hist.value_at_percentile(95.0);
let p99  = hist.value_at_percentile(99.0);
let avg  = hist.mean();
let min  = hist.min();
let max  = hist.max();
let cnt  = hist.len();

// Histogram buckets for chart (feeds plotters Histogram::vertical):
for v in hist.iter_recorded() {
    // v.value_iterated_to() -> bucket upper bound
    // v.count_at_value()    -> count in this bucket
}
```

**Why `hdrhistogram` over alternatives:**

| Crate | Version | Verdict | Reason |
|---|---|---|---|
| `hdrhistogram` | 7.5.4 | **Use this** | Battle-tested (Tokio, criterion, Prometheus clients); O(1) record; `value_at_percentile()` is clean and direct; `iter_recorded()` produces bucket data directly consumable by plotters |
| `histogram` | 1.3.1 | Reject | Active API churn: `percentile()` is deprecated in favor of `SampleQuantiles` trait; less adoption than hdrhistogram |
| `hstats` | unverified | Reject | Very low adoption; `bins_at_centiles()` returns approximate midpoints, not bucket iterator suitable for plotters |

**Usage pattern:** Project is single-threaded streaming. One `Histogram<u64>` per
template key in the aggregation map. Records accumulate during the streaming pass.
Percentile and bucket queries run once per template during output. No concurrency
primitives needed — `hdrhistogram` supports this perfectly.

**Dependencies of `hdrhistogram`:** `byteorder`, `num-traits` — both pure Rust,
no system deps, no C libraries.

**Confidence:** HIGH — verified via official docs at
[docs.rs/hdrhistogram](https://docs.rs/hdrhistogram/latest/hdrhistogram/).

---

## 3. SVG Chart Generation

### Recommendation: `plotters` 0.3.7 (SVG-only feature flags)

```toml
[dependencies]
plotters = { version = "0.3", default-features = false, features = [
    "svg_backend",
    "line_series",
    "histogram",
    "full_palette",
    "all_elements",
] }
```

**Why this feature set:**
- `default-features = false` eliminates `bitmap_backend`, `ttf` (font-kit, freetype),
  and image encoding — removes all system dependencies
- `svg_backend` only adds `plotters-svg 0.3.7` + `plotters-backend 0.3.6`, both
  pure Rust, no system deps
- `all_elements` is required for the `Pie` element (CHART-05)
- SVG output uses `<path>` and `<text>` elements with referenced font names (no
  embedded fonts), so no font system dependency
- Compile time: stated as < 6 seconds for SVG-only configuration

**Chart type coverage (all four CHART-0x requirements):**

| Chart | Requirement | Plotters API |
|---|---|---|
| Top N template frequency bar chart | CHART-02 | `Histogram::vertical()` with `(0u32..N).into_segmented()` axis |
| Execution time distribution histogram | CHART-03 | `Histogram::vertical()` fed with bucket data from `hdrhistogram::iter_recorded()` |
| SQL execution trend line chart | CHART-04 | `LineSeries::new()` with `build_cartesian_2d()` on a datetime/numeric x-axis |
| User/Schema pie chart | CHART-05 | `Pie::new(&center, &radius, &sizes, &colors, &labels)` element |

**Verified code patterns (from Context7):**

Bar/histogram chart:
```rust
use plotters::prelude::*;
let root = SVGBackend::new("chart.svg", (1024, 600)).into_drawing_area();
root.fill(&WHITE)?;
let mut chart = ChartBuilder::on(&root)
    .caption("Top N Templates", ("sans-serif", 30))
    .margin(10)
    .x_label_area_size(35)
    .y_label_area_size(50)
    .build_cartesian_2d((0u32..top_n).into_segmented(), 0u64..max_count)?;
chart.configure_mesh().draw()?;
chart.draw_series(
    Histogram::vertical(&chart)
        .style(BLUE.mix(0.6).filled())
        .margin(2)
        .data(template_counts.iter().enumerate().map(|(i, &c)| (i as u32, c))),
)?;
root.present()?;
```

Pie chart:
```rust
let dims = root.dim_in_pixel();
let center = (dims.0 as i32 / 2, dims.1 as i32 / 2);
let mut pie = Pie::new(&center, &250.0f64, &sizes, &colors, &labels);
pie.start_angle(0.0);
pie.label_style((("sans-serif", 16).into_font()).color(&BLACK));
root.draw(&pie)?;
```

**Why not alternatives:**

| Option | Verdict | Reason |
|---|---|---|
| `charts-rs` | Reject | Pulls font rendering + image crates; heavier; lower adoption |
| `charming` | Reject | Renders via ECharts JS renderer; not standalone SVG file output |
| Hand-written SVG strings | Reject | Pie chart arc geometry is error-prone to compute manually; plotters handles this correctly |
| `svg` crate (low-level) | Reject | No chart primitives; all bar/line/pie logic would need to be built from scratch |
| `plotlib` | Reject | No active development; no pie chart support |

**Known limitation:** SVG text labels use the viewer's system default sans-serif font
(not embedded). This is acceptable for CLI tooling output — the charts are
informational, not print-quality reports.

**Confidence:** HIGH — verified via Context7 with working code examples for
`SVGBackend`, `Histogram::vertical`, `LineSeries`, and `Pie` elements.

---

## Cargo.toml Changes

Add to `[dependencies]`:

```toml
hdrhistogram = "7.5"
plotters = { version = "0.3", default-features = false, features = [
    "svg_backend",
    "line_series",
    "histogram",
    "full_palette",
    "all_elements",
] }
```

No removals. No changes to existing dependencies.

---

## Integration Points with Existing Code

### SQL Normalization

- **File:** `src/features/sql_fingerprint.rs` — add `normalize_template(sql: &str) -> String`
  alongside existing `fingerprint()`
- **Usage:** New pipeline processor `TemplateAggregator` calls `normalize_template()`
  to derive the map key, then looks up or inserts `TemplateStats` in
  `ahash::HashMap<String, TemplateStats>` (ahash already in Cargo.toml)
- The `exec_time_ms` field (index 11 in `FIELD_NAMES`) is the value to aggregate

### Percentile Stats

- **Struct:** `TemplateStats { count: u64, hist: Histogram<u64> }` — one per template key
- The `Histogram<u64>` accumulates during the streaming pass (hot loop via processor)
- Queries (`value_at_percentile`, `iter_recorded`) run once per template in the output
  phase, after all records are processed
- This is outside the hot loop — no performance impact on existing streaming path

### SVG Charts

- **New module:** `src/chart/` (or `src/features/chart.rs`)
- Takes `Vec<TemplateStats>` after streaming completes
- Writes SVG files to the config-specified output directory (`[chart] output_dir`)
- Runs once after all records are processed — zero impact on the streaming hot loop

---

## What NOT to Add

| Crate | Why not |
|---|---|
| `sqlparser` | Full parser overhead; no DaMeng dialect; required transforms are simpler as byte-walks |
| `sql-fingerprint` | Depends on sqlparser; collapses identifiers too aggressively for template identity |
| `charts-rs` | Font + image deps violate no-system-dep constraint |
| `charming` | Wrong output model (JS renderer, not standalone SVG) |
| `ndarray` or `statrs` | Overkill; `hdrhistogram` provides all needed: mean, min, max, percentiles, histogram buckets |
| `rayon` (new uses in charts) | Chart generation is once-after-streaming; parallelism adds complexity for zero streaming-path gain |

---

## Confidence Assessment

| Area | Level | Reason |
|---|---|---|
| SQL normalization (DIY) | HIGH | Existing code read and understood; byte-walk pattern is established |
| `hdrhistogram` API | HIGH | Verified via official docs; `value_at_percentile()` and `iter_recorded()` confirmed |
| `plotters` SVG-only config | HIGH | Context7 docs verified all four chart types; Cargo.toml feature flags confirmed |
| DaMeng dialect incompatibility | HIGH | sqlparser dialect list reviewed; no DaMeng entry exists |
| `sql-fingerprint` over-collapse | HIGH | Docs explicitly state "identifier and value lists reduced to '…'" |

---

## Sources

- plotters docs via Context7 — `SVGBackend::new`, `Histogram::vertical`, `Pie::new`,
  `LineSeries`, `ChartBuilder`: HIGH confidence
- plotters 0.3.7 Cargo.toml (feature deps confirmed):
  [https://docs.rs/crate/plotters/latest/source/Cargo.toml.orig](https://docs.rs/crate/plotters/latest/source/Cargo.toml.orig)
- hdrhistogram docs — `value_at_percentile()`, `iter_recorded()`, `mean()`, `min()`, `max()`:
  [https://docs.rs/hdrhistogram/latest/hdrhistogram/](https://docs.rs/hdrhistogram/latest/hdrhistogram/)
- sql-fingerprint 1.11.1 sole dependency confirmed (sqlparser >= 0.62.0):
  [https://docs.rs/crate/sql-fingerprint/latest/source/Cargo.toml.orig](https://docs.rs/crate/sql-fingerprint/latest/source/Cargo.toml.orig)
- sqlparser 0.62.0 (released 2026-05-07) — round-trip normalization via `Display`,
  no DaMeng dialect:
  [https://docs.rs/sqlparser/latest/sqlparser/](https://docs.rs/sqlparser/latest/sqlparser/)
- histogram crate 1.3.1 — SampleQuantiles API (deprecation noted):
  [https://docs.rs/histogram/latest/histogram/](https://docs.rs/histogram/latest/histogram/)

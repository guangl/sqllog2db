# Performance Baselines

Recorded on branch `main`, commit date 2026-04-04 (v0.5.0).
Machine: Apple Silicon (Darwin 25.3.0), release build (`opt-level=z`, LTO, strip).
Synthetic log lines ≈ 170 bytes/record (realistic DaMeng SQL log format).
CSV/JSONL output goes to `/dev/null` (measures parse + serialization, no disk I/O).
SQLite output goes to a real file (includes fsync-off WAL-off single-transaction insert).

---

## How to reproduce

```bash
# Run a single bench (fast)
cargo bench --bench bench_csv     --features csv
cargo bench --bench bench_jsonl   --features jsonl
cargo bench --bench bench_sqlite  --features sqlite
cargo bench --bench bench_filters --features "filters,csv"

# Run all at once
cargo bench --all-features
```

## How to compare against this baseline

The baseline JSON data lives in `benches/baselines/`.
Criterion uses the `CRITERION_HOME` environment variable to locate it.

```bash
# Save a new named baseline after your changes
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv --features csv -- --save-baseline my-branch

# Compare your branch against the committed baseline
CRITERION_HOME=benches/baselines cargo bench --bench bench_csv --features csv -- --baseline baseline
```

Criterion prints a comparison like:

```
csv_export/10000  time: [2.48 ms 2.49 ms 2.50 ms]
                  change: [+0.1% +0.5% +0.9%] (p = 0.01 < 0.05)
                  No change in performance detected.
```

A regression is flagged when the change is statistically significant and positive.

---

## Baseline numbers

All timings are the median (middle of the `[low median high]` interval).
Throughput = records / median time.

### CSV export (→ /dev/null)

| Records | Median time | Throughput |
|--------:|------------:|-----------:|
|   1 000 |    267 µs   |  3.75 M/s  |
|  10 000 |   2.16 ms   |  4.63 M/s  |
|  50 000 |  11.24 ms   |  4.45 M/s  |

### JSONL export (→ /dev/null)

| Records | Median time | Throughput |
|--------:|------------:|-----------:|
|   1 000 |    402 µs   |  2.49 M/s  |
|  10 000 |   3.55 ms   |  2.81 M/s  |
|  50 000 |  17.82 ms   |  2.81 M/s  |

### SQLite export (→ bench.db, `JOURNAL_MODE=OFF SYNCHRONOUS=OFF`)

| Records | Median time | Throughput |
|--------:|------------:|-----------:|
|   1 000 |    891 µs   |  1.12 M/s  |
|  10 000 |   6.89 ms   |  1.45 M/s  |
|  50 000 |  34.61 ms   |  1.44 M/s  |

### Filter pipeline (10 000 records, CSV → /dev/null)

| Scenario              | Median time | Throughput    | Notes |
|-----------------------|------------:|--------------:|-------|
| `no_pipeline`         |   2.10 ms   |   4.75 M/s    | Fast path — no filter overhead |
| `pipeline_passthrough`|   2.77 ms   |   3.62 M/s    | All records pass; overhead = pipeline dispatch |
| `trxid_small`         |   1.08 ms   |   9.30 M/s    | 10 IDs in HashSet; ~0.1% pass → less export work |
| `trxid_large`         |   1.30 ms   |   7.70 M/s    | 1 000 IDs in HashSet; ~10% pass |
| `indicator_prescan`   |   2.12 ms   |   4.72 M/s    | Two-pass (pre-scan + main); `min_runtime_ms=2000` |

> **trxid filter throughput appears higher than `no_pipeline`** because the filter
> discards most records — the exporter does far less work even though the filter
> itself adds overhead.

---

## Performance rules

When adding a new feature, the following must hold (±5% tolerance for measurement noise):

| Benchmark | Hard limit |
|-----------|-----------|
| `csv_export/10000`       | ≤ 2.27 ms  |
| `jsonl_export/10000`     | ≤ 3.73 ms  |
| `sqlite_export/10000`    | ≤ 7.23 ms  |
| `filters/no_pipeline`    | ≤ 2.21 ms  |
| `filters/pipeline_passthrough` | ≤ 2.91 ms |

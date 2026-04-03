# Performance Baselines

Recorded on branch `bench/baseline`, commit date 2026-04-03.
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
|   1 000 |    297 µs   |  3.37 M/s  |
|  10 000 |   2.48 ms   |  4.03 M/s  |
|  50 000 |  12.46 ms   |  4.01 M/s  |

### JSONL export (→ /dev/null)

| Records | Median time | Throughput |
|--------:|------------:|-----------:|
|   1 000 |    543 µs   |  1.84 M/s  |
|  10 000 |   5.02 ms   |  1.99 M/s  |
|  50 000 |  25.24 ms   |  1.98 M/s  |

### SQLite export (→ bench.db, `JOURNAL_MODE=OFF SYNCHRONOUS=OFF`)

| Records | Median time | Throughput |
|--------:|------------:|-----------:|
|   1 000 |    863 µs   |  1.16 M/s  |
|  10 000 |   7.40 ms   |  1.35 M/s  |
|  50 000 |  37.70 ms   |  1.33 M/s  |

### Filter pipeline (10 000 records, CSV → /dev/null)

| Scenario              | Median time | Throughput    | Notes |
|-----------------------|------------:|--------------:|-------|
| `no_pipeline`         |   2.47 ms   |   4.05 M/s    | Fast path — no filter overhead |
| `pipeline_passthrough`|   3.09 ms   |   3.24 M/s    | All records pass; overhead = pipeline dispatch |
| `trxid_small`         |   1.11 ms   |   9.00 M/s    | 10 IDs in HashSet; ~0.1% pass → less export work |
| `trxid_large`         |   1.37 ms   |   7.32 M/s    | 1 000 IDs in HashSet; ~10% pass |
| `indicator_prescan`   |   2.41 ms   |   4.15 M/s    | Two-pass (pre-scan + main); `min_runtime_ms=2000` |

> **trxid filter throughput appears higher than `no_pipeline`** because the filter
> discards most records — the exporter does far less work even though the filter
> itself adds overhead.

---

## Performance rules

When adding a new feature, the following must hold (±5% tolerance for measurement noise):

| Benchmark | Hard limit |
|-----------|-----------|
| `csv_export/10000`       | ≤ 2.61 ms  |
| `jsonl_export/10000`     | ≤ 5.27 ms  |
| `sqlite_export/10000`    | ≤ 7.77 ms  |
| `filters/no_pipeline`    | ≤ 2.59 ms  |
| `filters/pipeline_passthrough` | ≤ 3.24 ms |

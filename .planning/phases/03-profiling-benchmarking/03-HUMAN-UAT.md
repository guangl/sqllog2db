---
status: complete
phase: 03-profiling-benchmarking
source: [03-VERIFICATION.md]
started: 2026-04-27T00:00:00Z
updated: 2026-04-27T00:00:00Z
---

## Current Test

[testing complete]

## Tests

### 1. flamegraph 符号可读性核验
expected: 在浏览器中打开 docs/flamegraphs/csv_export_real.json（samply 格式）后，可见 sqllog2db 内部函数名，无大量 [unknown] 帧；BENCHMARKS.md 记录的 Top 3 函数与实际火焰图一致
result: pass

## Summary

total: 1
passed: 1
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

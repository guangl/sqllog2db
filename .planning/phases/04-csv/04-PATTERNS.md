# Phase 4: CSV 性能优化 - Pattern Map

**Mapped:** 2026-04-27
**Files analyzed:** 3 个需修改文件 + 1 个需新增 group
**Analogs found:** 3 / 3（全部为本文件内自引用，无需外部 analog）

---

## File Classification

| 新增/修改文件 | Role | Data Flow | 最近 Analog | 匹配质量 |
|---|---|---|---|---|
| `src/exporter/csv.rs` | exporter（修改） | streaming / batch | 自身（`write_record_preparsed` 私有 → pub(crate)） | exact |
| `benches/bench_csv.rs` | benchmark（修改） | batch / transform | 自身（`bench_csv_export` group 新增 `bench_csv_format_only`） | exact |
| `src/config.rs` | config（修改，兜底 D-05/D-06） | — | 自身（`CsvExporter` struct 新增 `include_performance_metrics` 字段） | exact |

> 注：Phase 4 不新增独立文件，全部为现有文件的精准改动。

---

## Pattern Assignments

### 1. `src/exporter/csv.rs` — `write_record_preparsed` 可见性修改

**任务：** 将 `write_record_preparsed` 从 `fn`（私有）改为 `pub(crate)`，使 `benches/bench_csv.rs` 可直接调用，隔离格式化层开销。

**当前签名**（行 77–90）：
```rust
#[inline]
fn write_record_preparsed(
    itoa_buf: &mut itoa::Buffer,
    line_buf: &mut Vec<u8>,
    sqllog: &Sqllog<'_>,
    meta: &MetaParts<'_>,
    pm: &PerformanceMetrics<'_>,
    writer: &mut BufWriter<File>,
    path: &Path,
    normalize: bool,
    normalized_sql: Option<&str>,
    field_mask: crate::features::FieldMask,
    ordered_indices: &[usize],
) -> Result<()> {
```

**修改后（仅改可见性修饰符）：**
```rust
#[inline]
pub(crate) fn write_record_preparsed(   // fn → pub(crate)
    itoa_buf: &mut itoa::Buffer,
    // ... 其余参数不变
```

**参考：** 同文件 `normalize` 和 `field_mask` 字段已是 `pub(crate)`（行 31–33），遵循相同约定：
```rust
pub(crate) normalize: bool,
pub(crate) field_mask: crate::features::FieldMask,
pub(crate) ordered_indices: Vec<usize>,
```

**注意事项：**
- 仅改可见性，函数体、签名、`#[inline]` 均不变
- `clippy --all-targets -D warnings` 须通过（bench crate 引用此函数后不再"dead_code"）

---

### 2. `benches/bench_csv.rs` — 新增 `bench_csv_format_only` group

**任务：** 新增一个只测格式化（不含 `parse_meta` / `parse_performance_metrics`）的 micro-benchmark group，直接调用 `CsvExporter::write_record_preparsed`，隔离格式化层净开销。

**现有 benchmark 结构**（行 57–126，供复制模式）：

```rust
// 现有 group 的 throughput + bench_with_input 模式
fn bench_csv_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_export");

    for &n in &[1_000usize, 10_000, 50_000] {
        // ...
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &cfg, |b, cfg| {
            b.iter(|| { /* ... */ });
        });
    }

    group.finish();
}
```

**新增 group 模式（照搬以下结构）：**

```rust
fn bench_csv_format_only(c: &mut Criterion) {
    use dm_database_sqllog2db::exporter::csv::CsvExporter;
    use dm_database_parser_sqllog::LogParser;

    // 预先构造 Sqllog 记录，在 iter() 内只跑格式化，排除解析噪声
    // 硬编码典型记录（D-03）：ts, ep, trxid, sql 等字段
    const LOG_LINE: &str =
        "2024-01-01 00:00:00.000 (EP[1234] sess:0x0001 user:BENCHUSER trxid:TID001 stmt:0x1 appname:App ip:10.0.0.1) [SEL] SELECT * FROM t WHERE id = 1. EXECTIME: 10(ms) ROWCOUNT: 1(rows) EXEC_ID: 1.\n";

    let tmp = tempfile::TempDir::new().unwrap();
    let log_path = tmp.path().join("fmt.log");
    // 写入 10000 条相同记录
    let content: String = LOG_LINE.repeat(10_000);
    std::fs::write(&log_path, &content).unwrap();

    let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
    let records: Vec<_> = parser.iter().filter_map(|r| r.ok()).collect();
    // 预解析 meta + pm，benchmark iter() 内只跑格式化
    let parsed: Vec<_> = records
        .iter()
        .map(|r| (r, r.parse_meta(), r.parse_performance_metrics()))
        .collect();

    const N: usize = 10_000;
    let out = tmp.path().join("out.csv");

    let mut group = c.benchmark_group("csv_format_only");
    group.throughput(Throughput::Elements(N as u64));
    group.bench_function(BenchmarkId::from_parameter(N), |b| {
        b.iter(|| {
            let mut exporter = CsvExporter::new(&out);
            exporter.initialize().unwrap();
            for (sqllog, meta, pm) in &parsed {
                CsvExporter::write_record_preparsed(
                    &mut itoa::Buffer::new(),
                    &mut Vec::with_capacity(2048),
                    sqllog,
                    meta,
                    pm,
                    // writer 需从 exporter 取出，或通过 export_one_preparsed 路径
                    // 详见下方"访问策略"说明
                    ...
                ).unwrap();
            }
            exporter.finalize().unwrap();
        });
    });
    group.finish();
}
```

**访问策略（两种选择，按 Wave 0 实测决定）：**

选项 A — 直接调用 `pub(crate) write_record_preparsed`（需 bench crate 在同一 crate 内，即 `src/lib.rs` 暴露）。

选项 B — 通过已公开的 `export_one_preparsed` 路径调用（行 406–434），复用 `CsvExporter` 实例：
```rust
// 选项 B：更简单，但包含 Option::unwrap + writer borrow 开销
exporter.export_one_preparsed(sqllog, meta, pm, None).unwrap();
```

**推荐：** Wave 0 先用选项 B 量化，若噪声过高再切换选项 A（需 `pub(crate)` 修改）。

**criterion_group 注册**（行 125，修改为）：
```rust
criterion_group!(benches, bench_csv_export, bench_csv_real_file, bench_csv_format_only);
criterion_main!(benches);
```

**Baseline 路径层级说明（Pitfall 4 防范）：**
- `csv_format_only` group 无 v1.0 baseline，**不要**用 `--baseline v1.0` 对比它
- 只对 `csv_export` 和 `csv_export_real` group 做 `--baseline v1.0` 对比
- Wave 0 完成后可用 `--save-baseline wave0` 为新 group 建立自身基线

---

### 3. `src/config.rs` — 兜底配置项 `include_performance_metrics`（仅在 D-05 触发时实施）

**触发条件（D-05）：** Wave 0 格式化层优化后吞吐提升 < 10%，才实施此项。

**任务：** 在 `CsvExporter` config struct 新增 `include_performance_metrics: bool`，默认 `true`，关闭时跳过 `parse_performance_metrics()` 调用，并在 CSV 中省略 exectime/rowcount/exec_id 三个性能指标字段。

**现有 `CsvExporter` struct**（行 299–316）：
```rust
#[derive(Debug, Deserialize, Clone)]
pub struct CsvExporter {
    pub file: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    #[serde(default)]
    pub append: bool,
}
```

**修改后模式（照搬 `overwrite` 字段的 `default_true` 模式）：**
```rust
#[derive(Debug, Deserialize, Clone)]
pub struct CsvExporter {
    pub file: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    #[serde(default)]
    pub append: bool,
    /// 关闭时跳过 parse_performance_metrics()，CSV 省略 exectime/rowcount/exec_id 三列。
    /// 默认 true，保持现有行为不变。
    #[serde(default = "default_true")]
    pub include_performance_metrics: bool,
}
```

**Default impl 补充（照搬现有 Default 模式，行 308–316）：**
```rust
impl Default for CsvExporter {
    fn default() -> Self {
        Self {
            file: "outputs/sqllog.csv".to_string(),
            overwrite: true,
            append: false,
            include_performance_metrics: true,  // 新增，默认 true
        }
    }
}
```

**`apply_one` 新增 key（行 116–174，照搬 csv.overwrite 的 parse_bool 模式）：**
```rust
"exporter.csv.include_performance_metrics" => {
    self.exporter
        .csv
        .get_or_insert_with(Default::default)
        .include_performance_metrics = parse_bool(value)?;
}
```

**`cli/run.rs` 热循环接入点（`process_log_file`，行 176 附近）：**
```rust
// 当前：
let pm = record.parse_performance_metrics();

// 修改后（include_performance_metrics=false 时跳过）：
// 需将配置值传入 process_log_file，或在 ExporterManager 层判断
// 具体接入位置待 Wave 0 量化后决定
```

---

## Shared Patterns

### 可见性约定（pub(crate)）
**来源：** `src/exporter/csv.rs` 行 31–33
**应用到：** `write_record_preparsed` 的可见性修改
```rust
// 字段已有 pub(crate) 先例：
pub(crate) normalize: bool,
pub(crate) field_mask: crate::features::FieldMask,
pub(crate) ordered_indices: Vec<usize>,
```

### Config 布尔字段 + 默认值模式
**来源：** `src/config.rs` 行 299–329
**应用到：** `include_performance_metrics` 新字段
```rust
#[serde(default = "default_true")]
pub overwrite: bool,

fn default_true() -> bool {
    true
}
```

### apply_one parse_bool 模式
**来源：** `src/config.rs` 行 104–114，130–144
**应用到：** `--set exporter.csv.include_performance_metrics=false` 覆盖支持
```rust
let parse_bool = |v: &str| -> Result<bool> {
    match v {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(Error::Config(ConfigError::InvalidValue { ... })),
    }
};

"exporter.csv.overwrite" => {
    self.exporter
        .csv
        .get_or_insert_with(Default::default)
        .overwrite = parse_bool(value)?;
}
```

### Criterion group 结构
**来源：** `benches/bench_csv.rs` 行 57–88
**应用到：** 新增 `bench_csv_format_only` group
```rust
fn bench_csv_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_export");
    group.throughput(Throughput::Elements(n as u64));
    group.bench_with_input(BenchmarkId::from_parameter(n), &cfg, |b, cfg| {
        b.iter(|| { ... });
    });
    group.finish();
}
```

### 热循环快速路径条件判断模式
**来源：** `src/cli/run.rs` 行 159–165
**应用到：** 兜底方案中 `include_performance_metrics=false` 时跳过 `parse_performance_metrics()`
```rust
// 现有模式：pipeline.is_empty() 快速路径
let (passes, cached_meta) = if pipeline.is_empty() {
    (true, None)
} else {
    let meta = record.parse_meta();
    let ok = pipeline.run_with_meta(&record, &meta);
    (ok, Some(meta))
};
```

---

## No Analog Found

无：Phase 4 所有改动均在现有文件中，无需从零建立新模式。

---

## Wave 执行顺序

| Wave | 文件 | 动作 | 前置条件 |
|---|---|---|---|
| Wave 0 | `src/exporter/csv.rs` | `write_record_preparsed` → `pub(crate)` | 无 |
| Wave 0 | `benches/bench_csv.rs` | 新增 `bench_csv_format_only` group | `pub(crate)` 完成 |
| Wave 1 | 视 Wave 0 量化结果 | csv.rs 格式化层精准优化（如有收益） | Wave 0 benchmark 数据 |
| Wave 2（兜底） | `src/config.rs` + `src/cli/run.rs` | `include_performance_metrics` 配置项 | Wave 1 提升 < 10% |

---

## Metadata

**Analog search scope:** `src/exporter/`, `src/cli/`, `src/config.rs`, `benches/`
**Files scanned:** 4（csv.rs, run.rs, config.rs, bench_csv.rs）
**Pattern extraction date:** 2026-04-27

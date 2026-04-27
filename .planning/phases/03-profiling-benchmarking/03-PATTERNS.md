# Phase 3: Profiling & Benchmarking - Pattern Map

**Mapped:** 2026-04-26
**Files analyzed:** 3 new/modified files
**Analogs found:** 3 / 3

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `benches/bench_csv.rs` (扩展 real-file group) | benchmark | batch / file-I/O | `benches/bench_csv.rs` (现有合成部分) | exact |
| `benches/bench_sqlite.rs` (扩展 real-file group) | benchmark | batch / file-I/O | `benches/bench_sqlite.rs` (现有合成部分) | exact |
| `Cargo.toml` (新增 `[profile.flamegraph]`) | config | — | `Cargo.toml` `[profile.release]` 块 | exact |
| `benches/BENCHMARKS.md` (更新 v1.0 基准数值) | documentation | — | `benches/BENCHMARKS.md` 现有结构 | exact |

---

## Pattern Assignments

### `benches/bench_csv.rs` — 新增 `csv_export_real` benchmark group

**Analog:** `benches/bench_csv.rs`（现有 `bench_csv_export` 函数）

**Imports pattern** (`benches/bench_csv.rs` 第 1-12 行):
```rust
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dm_database_sqllog2db::cli::run::handle_run;
use dm_database_sqllog2db::config::Config;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
```
新增 real-file group 沿用完全相同的 import 块，无需额外导入。

**make_config pattern** (`benches/bench_csv.rs` 第 31-54 行):
```rust
fn make_config(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let toml = format!(
        r#"
[sqllog]
directory = "{sqllog}"

[error]
file = "{dir}/errors.log"

[logging]
file = "{dir}/app.log"
level = "warn"
retention_days = 1

[exporter.csv]
file = "/dev/null"
overwrite = true
append = false
"#,
        sqllog = sqllog_dir.to_string_lossy().replace('\\', "/"),
        dir = bench_dir.to_string_lossy().replace('\\', "/"),
    );
    toml::from_str(&toml).unwrap()
}
```
real-file group 复用同一 `make_config`，只需将 `sqllog_dir` 指向 `PathBuf::from("sqllogs")`。

**Core benchmark group pattern** (`benches/bench_csv.rs` 第 56-87 行):
```rust
fn bench_csv_export(c: &mut Criterion) {
    let bench_dir = PathBuf::from("target/bench_csv");
    let sqllog_dir = bench_dir.join("sqllogs");
    fs::create_dir_all(&sqllog_dir).unwrap();

    let mut group = c.benchmark_group("csv_export");

    for &n in &[1_000usize, 10_000, 50_000] {
        fs::write(sqllog_dir.join("bench.log"), synthetic_log(n)).unwrap();
        let cfg = make_config(&sqllog_dir, &bench_dir);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &cfg, |b, cfg| {
            b.iter(|| {
                handle_run(
                    cfg,
                    None,
                    false,
                    true, // quiet=true: 排除进度条 I/O 对吞吐量测量的干扰
                    &Arc::new(AtomicBool::new(false)),
                    80,
                    false,
                    None,
                    1,
                )
                .unwrap();
            });
        });
    }

    group.finish();
}
```

**real-file group 差异点（在此模式基础上修改）:**
- 函数名：`bench_csv_real_file`
- `benchmark_group` 名称：`"csv_export_real"`
- `sqllog_dir`：`PathBuf::from("sqllogs")`（真实目录，不写入合成数据）
- 存在性检查：若 `sqllogs/` 不存在则 `eprintln!` 并 `return`（CI skip 模式）
- `group.sample_size(10)`：真实文件慢，减少采样次数
- `group.measurement_time(Duration::from_secs(60))`：给足测量时间
- `Throughput`：若记录数未知则省略（只记录绝对时间），不强求
- `criterion_group!` 宏：追加 `bench_csv_real_file` 到现有 group 宏

**CI skip 模式（函数开头）:**
```rust
fn bench_csv_real_file(c: &mut Criterion) {
    let real_dir = PathBuf::from("sqllogs");
    if !real_dir.exists() {
        eprintln!("sqllogs/ not found, skipping real-file benchmark");
        return;
    }
    // ... 其余 benchmark 逻辑
}
```

---

### `benches/bench_sqlite.rs` — 新增 `sqlite_export_real` benchmark group

**Analog:** `benches/bench_sqlite.rs`（现有 `bench_sqlite_export` 函数）

**make_config pattern** (`benches/bench_sqlite.rs` 第 32-59 行):
```rust
fn make_config(sqllog_dir: &Path, bench_dir: &Path) -> Config {
    let toml = format!(
        r#"
[sqllog]
directory = "{sqllog}"

[error]
file = "{dir}/errors.log"

[logging]
file = "{dir}/app.log"
level = "warn"
retention_days = 1

[exporter.sqlite]
database_url = "{dir}/bench.db"
table_name = "sqllogs"
overwrite = true
append = false
"#,
        sqllog = sqllog_dir.to_string_lossy().replace('\\', "/"),
        dir = bench_dir.to_string_lossy().replace('\\', "/"),
    );
    toml::from_str(&toml).unwrap()
}
```
注意：SQLite 写真实文件（`bench.db`），不能用 `/dev/null`。

**Core benchmark group pattern** (`benches/bench_sqlite.rs` 第 61-94 行):
```rust
fn bench_sqlite_export(c: &mut Criterion) {
    let bench_dir = PathBuf::from("target/bench_sqlite");
    let sqllog_dir = bench_dir.join("sqllogs");
    fs::create_dir_all(&sqllog_dir).unwrap();

    let mut group = c.benchmark_group("sqlite_export");
    group.sample_size(20); // SQLite 较慢，减少迭代次数

    for &n in &[1_000usize, 10_000, 50_000] {
        // ... 与 bench_csv 相同的 bench_with_input 结构
    }
    group.finish();
}
```

**real-file group 差异点:**
- 函数名：`bench_sqlite_real_file`
- `benchmark_group` 名称：`"sqlite_export_real"`
- `sqllog_dir`：`PathBuf::from("sqllogs")`
- `bench_dir`：`PathBuf::from("target/bench_sqlite_real")`（避免与合成 bench.db 冲突）
- 同样需要 `real_dir.exists()` 检查
- `group.sample_size(5)`：实文件 + SQLite 双重慢，进一步减少采样

---

### `Cargo.toml` — 新增 `[profile.flamegraph]`

**Analog:** `Cargo.toml` `[profile.release]` 块（第 88-94 行）

**现有 release profile pattern** (`Cargo.toml` 第 88-94 行):
```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
```

**flamegraph profile 模式（在 release 之后追加）:**
```toml
[profile.flamegraph]
inherits = "release"
debug = true    # 保留 DWARF 符号，flamegraph 解析函数名所必需
strip = "none"  # 覆盖 release 的 strip = "symbols"，否则 flamegraph 全显示 unknown
```

关键点：`strip = "symbols"` 在 `[profile.release]` 中已设置，不加 `[profile.flamegraph]` 则 flamegraph 无符号。`inherits = "release"` 保留所有性能优化（LTO、opt-level=3 等），仅覆盖 strip 和 debug。

---

### `benches/BENCHMARKS.md` — 更新 v1.0 基准数值

**Analog:** 现有 `benches/BENCHMARKS.md` 结构（第 1-105 行）

**现有文档结构模式** (`benches/BENCHMARKS.md` 第 1-48 行):
文档由以下章节组成，v1.0 更新需遵循相同格式：
1. 机器信息头（branch、commit date、版本、硬件）
2. "How to reproduce" — `cargo bench` 命令
3. "How to compare against this baseline" — `CRITERION_HOME` + `--save-baseline` / `--baseline`
4. "Baseline numbers" — 各 exporter 的记录数/时间/吞吐量表格
5. "Performance rules" — 硬性时间上限表格

v1.0 更新时：
- 更新文档头部版本（`v0.5.0` → `v1.0`，commit date → 当前日期）
- 更新机器信息（`opt-level=z` → `opt-level=3`，当前实际 profile）
- 在"Baseline numbers"新增 `### Real-file export` 章节，记录 real-file benchmark 绝对时间
- 若无 Throughput，注明"仅记录绝对时间，记录数未预扫描"
- 更新"Performance rules"表格（移除 JSONL 行，因已无 bench_jsonl）

---

## Shared Patterns

### Criterion handle_run 调用签名
**Source:** `benches/bench_csv.rs` 第 70-81 行 / `benches/bench_sqlite.rs` 第 77-88 行
**Apply to:** 所有新增的 benchmark 函数

所有 bench 文件调用 `handle_run` 时参数顺序固定：
```rust
handle_run(
    cfg,
    None,      // output_path override（bench 中不覆盖）
    false,     // verbose
    true,      // quiet=true — 必须为 true，排除进度条 I/O 干扰计时
    &Arc::new(AtomicBool::new(false)),  // shutdown signal
    80,        // terminal width（进度条宽度，quiet 时无影响）
    false,     // dry_run
    None,      // filter override
    1,         // thread count
)
.unwrap();
```
`quiet=true` 是 benchmark 正确性的核心保证，不能省略。

### criterion_group! 宏追加模式
**Source:** `benches/bench_csv.rs` 第 89-90 行
**Apply to:** bench_csv.rs 和 bench_sqlite.rs

```rust
// 追加新函数到现有 group 宏（不新建 [[bench]] 条目）
criterion_group!(benches, bench_csv_export, bench_csv_real_file);
criterion_main!(benches);
```

### benchmark group 参数组合
**Source:** `benches/bench_sqlite.rs` 第 67-68 行 / `benches/bench_filters.rs` 第 150-151 行
**Apply to:** 所有 real-file benchmark group

```rust
// 慢 benchmark 的参数组合
group.sample_size(10);  // 减少采样次数（默认 100）
group.measurement_time(std::time::Duration::from_secs(60));  // 延长单次测量窗口
```

---

## No Analog Found

本 Phase 所有文件在现有代码库中均有直接对应，无需参考外部 pattern。

---

## Metadata

**Analog search scope:** `benches/` 目录（bench_csv.rs, bench_sqlite.rs, bench_filters.rs, BENCHMARKS.md），`Cargo.toml`
**Files scanned:** 5
**Pattern extraction date:** 2026-04-26

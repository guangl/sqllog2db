# Phase 14: Exporter 集成输出 - Research

**Researched:** 2026-05-16
**Domain:** Rust Exporter trait 扩展 + SQLite DDL + CSV 伴随文件写入
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-01:** `Exporter` trait 新增方法 `write_template_stats(&mut self, stats: &[TemplateStats], final_path: Option<&Path>) -> Result<()>`；`SqliteExporter`、`CsvExporter`、`DryRunExporter` 各自实现；`ExporterKind` 静态分发透传；`ExporterManager` 提供同签名的外部接口

**D-02:** `ExporterManager::write_template_stats()` 是 `run.rs` 的唯一调用点；两条路径（顺序/并行）调用位置均为 `exporter_manager.finalize()` 之后

**D-03:** `final_path: Option<&Path>` 参数解决并行路径的最终 CSV 路径问题：顺序路径传 `None`（`CsvExporter` 用 `self.path`），并行路径传 `Some(Path::new(&csv_cfg.file))`（`concat_csv_parts()` 完成后的最终输出路径）

**D-04:** `write_template_stats()` 失败 → 返回 `Err`，`run.rs` 向上传播，整体退出码非零；不降级为 warn-and-continue

**D-05:** `DryRunExporter::write_template_stats()` 为 no-op，只打 `info!` 摘要；不产生任何文件

**D-06:** `SqliteExporter::finalize()` 只执行 `COMMIT`，不关闭连接（`conn: Option<Connection>` 保留）；`write_template_stats()` 在同一连接上开新事务写 `sql_templates`；`SqliteExporter` drop 时连接自动关闭

**D-07:** `sql_templates` 表行为跟随主表 overwrite/append 语义：`overwrite=true` 时 DROP IF EXISTS + CREATE TABLE；`append=true` 时 CREATE TABLE IF NOT EXISTS（保留历史行，INSERT INTO 新行）

**D-08:** 表已存在且 overwrite 时，必须在 `write_template_stats()` 内 DROP 并重建，而非在 `initialize()` 阶段提前处理

**D-09:** `CsvExporter::write_template_stats()` 写入 `<basename>_templates.csv`：若 `final_path` 为 `Some(p)` 则用该路径推导，若为 `None` 则从 `self.path` 推导（stem + `_templates.csv`）

**D-10:** 伴随文件始终覆盖写入（模板统计是全量结果，无追加语义），不跟随主 CSV append 标志

**D-11:** `write_template_stats()` 调用在 `exporter_manager.finalize()` 之后，任何主导出提前终止时数据完整性由现有结构天然保证

### Claude's Discretion

- `sql_templates` 表列类型：`template_key TEXT NOT NULL PRIMARY KEY`，数值列 `INTEGER NOT NULL`，时间戳列 `TEXT NOT NULL`
- SQL 注入防护：`sql_templates` 为固定表名，不需要 ASCII 白名单；DDL 使用固定字面量
- 批量写入：统计条目数量远小于主记录，单事务批量 INSERT 即可，无需分批

### Deferred Ideas (OUT OF SCOPE)

- 独立 JSON 报告输出（TMPL-03）— Future Requirements v1.4+
- 独立 CSV 报告输出（TMPL-03b）— Future Requirements v1.4+
- 图表生成（CHART-01~05）— Phase 15/16

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TMPL-04 | SQLite 导出时自动写入 `sql_templates` 统计表；CSV 导出时自动生成 `*_templates.csv` 伴随文件 | 已验证现有 SqliteExporter.conn 在 finalize() 后仍可用（D-06）；CSV 伴随文件路径推导逻辑已设计（D-09）；run.rs 两处调用点已定位 |

</phase_requirements>

---

## Summary

Phase 14 是一个纯 Rust 内部集成任务——无外部依赖、不引入新 crate。核心工作是对现有 `Exporter` trait 新增第四生命周期段 `write_template_stats()`，并在三个具体 exporter 中各自实现，最后在 `run.rs` 的两条路径（顺序 ~L886、并行 ~L792）的正确位置插入调用。

**关键约束已被代码验证：**

1. `SqliteExporter::finalize()` [VERIFIED] 当前只执行 `conn.execute_batch("COMMIT;")` 而不 take/drop `self.conn`，因此 `write_template_stats()` 调用时 `self.conn` 仍为 `Some(Connection)`，可安全复用（D-06 天然满足，无需改动 `finalize()`）。
2. 并行路径的 `ExporterManager` 存在于每个 rayon task 内部，`em.finalize()` 在各 task 末尾调用（L579），任务完成后 `em` 被 drop——并行路径**没有**可供调用 `write_template_stats()` 的公共 `ExporterManager`。因此并行路径的 `write_template_stats` 必须在 `handle_run()` 主线程（`process_csv_parallel()` 返回后），以新创建的临时 `ExporterManager` 或直接对 `CsvExporter` 调用（见下文"并行路径特殊处理"）。
3. `TemplateStats` struct [VERIFIED] 已在 `src/features/template_aggregator.rs` 定义，10 个字段：`template_key, count, avg_us, min_us, max_us, p50_us, p95_us, p99_us, first_seen, last_seen`，单位均为微秒，时间戳为 String。

**Primary recommendation:** 修改 4 个文件（`exporter/mod.rs`、`exporter/sqlite.rs`、`exporter/csv.rs`、`cli/run.rs`），不引入新依赖，可在 2 个计划（plan）内完成。

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 写入 sql_templates 表 | Database/Storage (SqliteExporter) | — | 复用现有 rusqlite 连接，属于 persistence tier |
| 生成 *_templates.csv 伴随文件 | Exporter/Output (CsvExporter) | — | 与主 CSV 同目录，属于同一输出 tier |
| 调用时序控制 | Orchestration (run.rs) | — | finalize() 之后调用，保证数据完整性 |
| No-op dry-run | DryRunExporter | — | 保持 dry-run 语义一致性 |
| 统一外部接口 | ExporterManager | ExporterKind | 静态分发，零虚表开销 |

---

## Standard Stack

### Core（已在 Cargo.toml 中，无需新增）

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rusqlite | 现有 | SQLite DDL + INSERT | 项目已用，连接已打开 [ASSUMED] |
| std::fs / std::io | std | CSV 伴随文件写入 | 标准库，无额外依赖 |
| itoa | 现有 | 整数序列化（与主 CSV 一致） | 项目已用，零分配 [ASSUMED] |
| log | 现有 | info! / warn! 日志 | 项目标准日志 [ASSUMED] |

### 无需新增依赖

本 Phase 不引入任何新 crate。所有实现均基于现有依赖。

**Package Legitimacy Audit:** 不适用（本 Phase 不安装新外部包）。

---

## Architecture Patterns

### 数据流图

```
run.rs handle_run()
  │
  ├─ [顺序路径]
  │    exporter_manager.finalize()?           ← 主记录 COMMIT
  │    let stats = template_agg.map(finalize) ← Vec<TemplateStats>
  │    exporter_manager.write_template_stats(&stats, None)?
  │         └─ ExporterKind::Sqlite → SqliteExporter::write_template_stats
  │                  BEGIN; DROP/CREATE sql_templates; INSERT × N; COMMIT
  │         └─ ExporterKind::Csv → CsvExporter::write_template_stats
  │                  path = self.path.stem + "_templates.csv"
  │                  write header + rows
  │         └─ ExporterKind::DryRun → info! only, no file
  │
  └─ [并行路径]
       process_csv_parallel() → (processed_files, skipped, merged_agg)
         内部各 task: em.finalize() 已在 task 内调用
       concat_csv_parts() → 最终 output_path
       let stats = merged_agg.map(finalize) ← Vec<TemplateStats>
       // 需要一个 CsvExporter 代理来写伴随文件
       // 方案A: ExporterManager::write_template_stats 在并行路径传 Some(output_path)
       //        但此时没有 ExporterManager 实例
       // 方案B（推荐）: 直接构造轻量 CsvExporter 仅用于调用 write_template_stats
       //        OR: ExporterManager::write_template_stats_csv(stats, path) 自由函数
```

### 并行路径的关键设计决策

**问题：** 并行路径 `process_csv_parallel()` 返回后，主线程没有 `ExporterManager` 实例（各 rayon task 内的 `em` 在 task 末尾已 finalize 并 drop）。

**CONTEXT.md D-02 规定：** `ExporterManager::write_template_stats()` 是唯一调用点。

**解决方案（已在 CONTEXT.md D-03 锁定）：**

并行路径需在 `handle_run()` 中临时构造一个 `ExporterManager`（或直接使用 `CsvExporter`），传入 `final_path = Some(Path::new(&csv_cfg.file))`。最简洁实现：

```rust
// 并行路径（process_csv_parallel 返回后）
let stats = parallel_agg.map(TemplateAggregator::finalize);
if let Some(ref stats) = stats {
    // 构造一个临时 ExporterManager 仅用于写伴随文件
    // CsvExporter 的 path 字段用于推导 _templates.csv，但实际路径由 final_path 覆盖
    let mut tmp_csv = CsvExporter::new(Path::new(&csv_cfg.file));
    let mut tmp_em = ExporterManager::from_csv(tmp_csv);
    tmp_em.write_template_stats(stats, Some(Path::new(&csv_cfg.file)))?;
}
```

OR 更简洁：直接在 `ExporterManager` 新增一个 `write_template_stats_for_path()` 自由函数形式。但鉴于 D-02 要求统一接口，推荐上述方案。

### 推荐项目结构（修改点）

```
src/
├── exporter/
│   ├── mod.rs          ← Exporter trait 新增 write_template_stats()
│   │                      ExporterKind 新增透传 arm
│   │                      ExporterManager 新增外部方法
│   ├── sqlite.rs       ← SqliteExporter::write_template_stats() impl
│   └── csv.rs          ← CsvExporter::write_template_stats() impl
└── cli/
    └── run.rs          ← 两处插入 write_template_stats 调用
```

### Pattern 1: Exporter trait 第四段扩展

**What:** 在现有三段式（initialize → export_one_preparsed × N → finalize）之后新增第四段

**When to use:** run 结束后，主导出已 finalize，template stats 已可用

```rust
// Source: 基于现有 Exporter trait 模式（[ASSUMED] - 本地 codebase 结构）
pub trait Exporter {
    // ... 现有方法 ...
    fn write_template_stats(
        &mut self,
        stats: &[crate::features::TemplateStats],
        final_path: Option<&std::path::Path>,
    ) -> crate::error::Result<()> {
        // 默认实现：no-op（兼容不支持此方法的 exporter）
        let _ = (stats, final_path);
        Ok(())
    }
}
```

注意：提供默认实现（no-op）让 DryRunExporter 只需覆盖以加 `info!`，其他未来 exporter 也可以不实现。

### Pattern 2: SQLite write_template_stats 实现模式

```rust
// Source: 参照 SqliteExporter::initialize() 中的 DDL 模式（[ASSUMED]）
fn write_template_stats(
    &mut self,
    stats: &[TemplateStats],
    _final_path: Option<&Path>,
) -> Result<()> {
    let conn = self.conn.as_ref()
        .ok_or_else(|| Self::db_err("not initialized"))?;

    // D-07/D-08: overwrite 时 DROP + CREATE；append 时 CREATE IF NOT EXISTS
    if self.overwrite {
        conn.execute("DROP TABLE IF EXISTS sql_templates", [])
            .map_err(|e| Self::db_err(format!("drop sql_templates failed: {e}")))?;
    }
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sql_templates (
            template_key TEXT NOT NULL PRIMARY KEY,
            count        INTEGER NOT NULL,
            avg_us       INTEGER NOT NULL,
            min_us       INTEGER NOT NULL,
            max_us       INTEGER NOT NULL,
            p50_us       INTEGER NOT NULL,
            p95_us       INTEGER NOT NULL,
            p99_us       INTEGER NOT NULL,
            first_seen   TEXT NOT NULL,
            last_seen    TEXT NOT NULL
        )",
        [],
    ).map_err(|e| Self::db_err(format!("create sql_templates failed: {e}")))?;

    conn.execute_batch("BEGIN;")
        .map_err(|e| Self::db_err(format!("begin failed: {e}")))?;
    for s in stats {
        conn.execute(
            "INSERT INTO sql_templates VALUES (?,?,?,?,?,?,?,?,?,?)",
            rusqlite::params![
                s.template_key, s.count, s.avg_us, s.min_us, s.max_us,
                s.p50_us, s.p95_us, s.p99_us, s.first_seen, s.last_seen
            ],
        ).map_err(|e| Self::db_err(format!("insert sql_templates failed: {e}")))?;
    }
    conn.execute_batch("COMMIT;")
        .map_err(|e| Self::db_err(format!("commit sql_templates failed: {e}")))?;

    info!("sql_templates: {} rows written", stats.len());
    Ok(())
}
```

**重要：** `conn` 的借用规则——`conn.execute_batch("BEGIN;")` 持有不可变借用，但 `conn.execute()` 也只需不可变借用（rusqlite `Connection::execute` 接受 `&self`）。整个方法可以顺畅借用。

### Pattern 3: CSV 伴随文件写入模式

```rust
// Source: 参照 CsvExporter::initialize() 中的文件写入模式（[ASSUMED]）
fn write_template_stats(
    &mut self,
    stats: &[TemplateStats],
    final_path: Option<&Path>,
) -> Result<()> {
    // D-09: 路径推导
    let base_path = final_path.unwrap_or(&self.path);
    let stem = base_path.file_stem().unwrap_or_default();
    let companion = base_path.with_file_name(
        format!("{}_templates.csv", stem.to_string_lossy())
    );

    // D-10: 始终覆盖写入
    ensure_parent_dir(&companion).map_err(|e| {
        Error::Export(ExportError::WriteFailed {
            path: companion.clone(),
            reason: format!("create dir failed: {e}"),
        })
    })?;
    let file = std::fs::File::create(&companion).map_err(|e| {
        Error::Export(ExportError::WriteFailed {
            path: companion.clone(),
            reason: format!("create failed: {e}"),
        })
    })?;
    let mut writer = std::io::BufWriter::new(file);

    // 写表头
    writer.write_all(
        b"template_key,count,avg_us,min_us,max_us,p50_us,p95_us,p99_us,first_seen,last_seen\n"
    )?;

    // 写数据行（用 itoa 写数值字段，与主 CSV 一致）
    let mut itoa_buf = itoa::Buffer::new();
    for s in stats {
        // template_key 可能含逗号/引号，需 CSV 转义
        writer.write_all(b"\"")?;
        // write_csv_escaped 是 crate-private，可以 use super::write_csv_escaped 或复用逻辑
        // 或调用 pub(crate) 版本
        ...
        writer.write_all(b"\",")?;
        writer.write_all(itoa_buf.format(s.count).as_bytes())?;
        // ... 其余数值字段
    }
    writer.flush()?;
    info!("Template companion CSV written: {}", companion.display());
    Ok(())
}
```

**注意事项：** `write_csv_escaped()` 在 `csv.rs` 中是模块级私有 fn（非 pub）。伴随文件写入在同一文件中，可直接调用。

### Anti-Patterns to Avoid

- **在 finalize() 中关闭 SQLite 连接：** `finalize()` 当前不 take `self.conn`（只 `execute_batch("COMMIT;")`），这是 D-06 依赖的前提条件。禁止改动 `finalize()` 使其 take/drop `self.conn`。
- **在 initialize() 阶段 DROP sql_templates：** D-08 明确要求在 `write_template_stats()` 内处理，因为 initialize 时 stats 不可用。
- **CSV 伴随文件跟随 append 标志：** D-10 要求始终覆盖写入，不论主 CSV 是否为 append 模式。
- **在 ExporterKind 的 write_template_stats 中对 DryRun 分支做任何文件 I/O。**

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CSV 转义 | 自定义转义函数 | 复用现有 `write_csv_escaped()` | 已有 memchr 优化实现 |
| SQLite 错误映射 | 自定义 error 类型 | 复用 `Self::db_err()` | 现有辅助函数 |
| 目录创建 | `std::fs::create_dir_all` 直接调用 | `ensure_parent_dir()` from `exporter/mod.rs` | 现有辅助函数 |
| 路径推导 | 字符串拼接 | `Path::with_file_name()` + `file_stem()` | 标准库路径操作 |

---

## Common Pitfalls

### Pitfall 1: `rusqlite::Connection` 借用冲突

**What goes wrong:** `let conn = self.conn.as_ref().unwrap()` 的不可变借用与同方法内的 `self.overwrite` 等字段访问可能产生借用冲突。

**Why it happens:** Rust 借用检查器在方法内不允许同时持有 `&self.conn`（通过 `as_ref()`）和通过 `self.overwrite` 读取其他字段（虽然只读，但 `self` 被部分借用）。

**How to avoid:** 先读取所需标志（`let overwrite = self.overwrite`）再借用 conn，或在独立作用域内完成 DDL：

```rust
let overwrite = self.overwrite;
let conn = self.conn.as_ref().ok_or_else(...)?;
if overwrite { ... }
```

**Warning signs:** 编译错误 "cannot borrow `self` as immutable because it is also borrowed as mutable"。

### Pitfall 2: 并行路径无 ExporterManager 实例

**What goes wrong:** 并行路径中试图在 `process_csv_parallel()` 内部或 task 里调用 `write_template_stats()`，但此时 stats 尚未 finalize。

**Why it happens:** `TemplateAggregator::finalize()` 只能在所有 rayon task 的 `merge()` 完成后调用，即 `process_csv_parallel()` 返回后。

**How to avoid:** 必须在 `handle_run()` 主线程、`process_csv_parallel()` 返回后构建调用点（见 Architecture Patterns 中的并行路径设计）。

### Pitfall 3: `stats` 为空时不应 skip

**What goes wrong:** 当 `template_analysis.enabled = false` 时 `template_agg` 为 `None`，`template_stats` 也为 `None`，`write_template_stats` 不会被调用——这是正确行为。但若 `enabled = true` 且无任何记录被观测（stats 为空 Vec），仍应调用 `write_template_stats()`（只是写入空表/空文件），不应提前 return。

**How to avoid:** `if let Some(ref stats) = template_stats { exporter_manager.write_template_stats(stats, ...)? }` 已天然处理 None 情况；空 Vec 情况下写入空表/空文件是正确语义。

### Pitfall 4: SQLite EXCLUSIVE locking 在 write_template_stats 后未释放

**What goes wrong:** SQLite 以 `PRAGMA locking_mode = EXCLUSIVE` 打开，`write_template_stats()` 完成后连接仍持有锁直到 drop。测试代码在 exporter drop 之前尝试读取数据库时会 SQLITE_BUSY。

**How to avoid:** 测试中必须用 `{ let mut e = ...; e.write_template_stats(...); }` 作用域确保 drop 后再开新连接验证。现有测试（如 `test_sqlite_basic_export`）已使用此模式。

### Pitfall 5: CSV 伴随文件路径推导对 `output.csv.gz` 类路径的处理

**What goes wrong:** `Path::file_stem()` 对 `output.csv.gz` 返回 `output.csv`，则伴随文件为 `output.csv_templates.csv` 而非 `output_templates.csv`。

**How to avoid:** 现有 CSV exporter 的 path 均为 `.csv` 结尾，`file_stem()` 返回 stem 不含 `.csv`。此 edge case 在当前项目中不会触发，但可加注释说明。

---

## Code Examples

### 现有 finalize() 实现（SQLite，仅 COMMIT）

```rust
// Source: src/exporter/sqlite.rs L397-L407 [VERIFIED: codebase read]
fn finalize(&mut self) -> Result<()> {
    if let Some(conn) = &self.conn {
        conn.execute_batch("COMMIT;")
            .map_err(|e| Self::db_err(format!("commit failed: {e}")))?;
    }
    info!(
        "SQLite export finished: {} (success: {}, failed: {})",
        self.database_url, self.stats.exported, self.stats.failed
    );
    Ok(())
}
```

重要：`conn` 是 `&self.conn`（不可变引用），`self.conn` 仍为 `Some(conn)` 状态，`write_template_stats` 可以继续使用。

### 现有 process_csv_parallel() 返回结构

```rust
// Source: src/cli/run.rs L476 [VERIFIED: codebase read]
// 返回类型：Result<(Vec<(PathBuf, usize)>, usize, Option<TemplateAggregator>)>
//           (已处理文件列表,           跳过数, 合并后聚合器)
let (processed_files, parallel_skipped, parallel_agg) = process_csv_parallel(...)?;
let template_stats = parallel_agg.map(TemplateAggregator::finalize);
// template_stats: Option<Vec<TemplateStats>>
```

### 顺序路径当前 finalize 调用点（需在此后插入）

```rust
// Source: src/cli/run.rs L886-L895 [VERIFIED: codebase read]
exporter_manager.finalize()?;
if !quiet {
    exporter_manager.log_stats();
}

// Phase 14 将消费 finalize() 结果并写出报告；此处先记录聚合摘要。
let template_stats = template_agg.map(TemplateAggregator::finalize);
if let Some(ref stats) = template_stats {
    info!("Template analysis: {} unique templates", stats.len());
    // Phase 14 在此插入：
    // exporter_manager.write_template_stats(stats, None)?;
}
```

### 并行路径当前 finalize 时序

```rust
// Source: src/cli/run.rs L771-L795 [VERIFIED: codebase read]
let (processed_files, parallel_skipped, parallel_agg) = process_csv_parallel(...)?;
// concat_csv_parts 在 process_csv_parallel 内部已完成
// parallel_agg 是所有 rayon task 聚合器 merge 后的结果
let template_stats = parallel_agg.map(TemplateAggregator::finalize);
if let Some(ref stats) = template_stats {
    info!("Template analysis: {} unique templates", stats.len());
    // Phase 14 在此插入：
    // 需要一个 ExporterManager 或直接 CsvExporter 来写伴随文件
    // csv_cfg.file 是最终输出路径（concat 之后）
}
```

---

## Runtime State Inventory

本 Phase 为新增功能（非重命名/重构），不适用 Runtime State Inventory。

---

## Environment Availability

本 Phase 无外部 CLI 工具依赖，仅依赖 Rust 标准工具链与 Cargo.toml 中已有的 crate。

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo / rustc | 构建 | 假定可用 | — | — |
| rusqlite | SqliteExporter | 现有 Cargo.toml | — | — |
| itoa | CSV 伴随文件数值写入 | 现有 Cargo.toml | — | — |

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust 内置 `#[test]` + tempfile |
| Config file | 无独立配置文件 |
| Quick run command | `cargo test` |
| Full suite command | `cargo test && cargo clippy --all-targets -- -D warnings` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TMPL-04-A | SQLite run 后数据库存在 sql_templates 表，含全部 10 列 | integration | `cargo test test_sqlite_write_template_stats` | ❌ Wave 0 |
| TMPL-04-B | CSV run 后生成 `<stem>_templates.csv` 伴随文件，含表头和数据行 | integration | `cargo test test_csv_write_template_stats` | ❌ Wave 0 |
| TMPL-04-C | write_template_stats 在 finalize 之后调用（顺序路径） | unit | `cargo test test_write_after_finalize_seq` | ❌ Wave 0 |
| TMPL-04-D | template_analysis.enabled=false 时不创建 sql_templates 表/伴随文件 | integration | `cargo test test_no_template_stats_when_disabled` | ❌ Wave 0 |
| TMPL-04-E | overwrite=true 时 sql_templates 表被重建（旧数据消失） | integration | `cargo test test_sqlite_templates_overwrite` | ❌ Wave 0 |
| TMPL-04-F | append=true 时 sql_templates 新增行（旧行保留） | integration | `cargo test test_sqlite_templates_append` | ❌ Wave 0 |
| TMPL-04-G | DryRunExporter::write_template_stats 为 no-op | unit | `cargo test test_dry_run_write_template_stats_noop` | ❌ Wave 0 |
| TMPL-04-H | 并行路径生成伴随文件（final_path=Some(...)） | integration | `cargo test test_parallel_csv_companion_file` | ❌ Wave 0 |

### Sampling Rate

- **每次 commit:** `cargo test`
- **每次 wave merge:** `cargo test && cargo clippy --all-targets -- -D warnings`
- **Phase gate:** 全套测试绿色后方可执行 `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/exporter/sqlite.rs` 中新增 `test_sqlite_write_template_stats`、`test_sqlite_templates_overwrite`、`test_sqlite_templates_append`
- [ ] `src/exporter/csv.rs` 中新增 `test_csv_write_template_stats`、`test_parallel_csv_companion_file`
- [ ] `src/exporter/mod.rs` 中新增 `test_dry_run_write_template_stats_noop`
- [ ] `src/cli/run.rs` 中新增集成测试 `test_no_template_stats_when_disabled`

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | 低风险 | `sql_templates` 为固定表名，无用户输入进入 DDL；`template_key` 通过 `params![]` 参数化绑定，rusqlite 自动转义 |
| V6 Cryptography | 否 | — |

**SQL 注入防护（[VERIFIED: codebase read]）：** 现有 `table_name` 使用 ASCII 白名单 + DDL 双引号转义（Phase 7 实现）。`sql_templates` 为固定字面量，不受用户输入影响，无需白名单。INSERT 中 `template_key` 值通过 `rusqlite::params![]` 参数化绑定，无 SQL 注入风险。

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 三段式 Exporter 生命周期 | 新增第四段 write_template_stats | Phase 14 | 无性能影响（finalize 后调用，不在热路径） |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | rusqlite、itoa 在 Cargo.toml 中已存在（未执行 cargo metadata 验证） | Standard Stack | 若缺失需补充依赖，但几乎确定存在 |
| A2 | `write_csv_escaped()` 在 `csv.rs` 中对 `write_template_stats()` 可见（同模块） | Code Examples | 若为私有则需在伴随文件写入逻辑中内联转义 |

---

## Open Questions

1. **并行路径的 `write_template_stats` 调用者构造**
   - What we know: 并行路径无公共 `ExporterManager` 实例；`csv_cfg.file` 是最终路径
   - What's unclear: 是在 `handle_run()` 临时创建 `ExporterManager::from_csv(CsvExporter::new(output_path))`，还是直接将 `CsvExporter::write_template_stats` 暴露为关联函数
   - Recommendation: 临时创建 `ExporterManager` 最符合 D-02（唯一调用点通过 ExporterManager），且代码量最小；ExporterManager 不需要 initialize/finalize，直接调用 write_template_stats 即可

2. **`write_template_stats` 是否需要默认实现（no-op）**
   - What we know: DryRunExporter 需要 no-op + info!；未来可能有新 exporter
   - What's unclear: 默认实现 no-op vs 强制每个 exporter 实现
   - Recommendation: 提供默认实现 `Ok(())`，DryRunExporter 覆盖以加 `info!`，向前兼容

---

## Sources

### Primary (HIGH confidence)

- `src/exporter/mod.rs` — `Exporter` trait、`ExporterKind`、`ExporterManager`、`DryRunExporter` 完整结构（codebase read）
- `src/exporter/sqlite.rs` — `SqliteExporter` 完整实现，包括 `conn: Option<Connection>`、`finalize()` 只 COMMIT 不 drop conn（codebase read）
- `src/exporter/csv.rs` — `CsvExporter` 完整实现，包括 `write_csv_escaped()`、`ensure_parent_dir()`（codebase read）
- `src/cli/run.rs` — 两条路径（顺序 L808-L896、并行 L761-L807）完整调用流程（codebase read）
- `src/features/template_aggregator.rs` — `TemplateStats` struct 字段定义（codebase read）
- `.planning/phases/14-exporter/14-CONTEXT.md` — 全部锁定决策 D-01~D-11（codebase read）

### Secondary (MEDIUM confidence)

- `.planning/REQUIREMENTS.md` TMPL-04 — 需求原文
- `.planning/ROADMAP.md` Phase 14 成功标准

---

## Metadata

**Confidence breakdown:**

- Standard Stack: HIGH — 全部为现有依赖，无新引入
- Architecture: HIGH — 基于实际代码阅读，并行路径问题已定位且有明确解法
- Pitfalls: HIGH — 均来自实际代码结构分析（借用规则、EXCLUSIVE lock、空 stats 等）

**Research date:** 2026-05-16
**Valid until:** 2026-06-16（codebase 变更时失效）

# sqllog2db

[![Crates.io](https://img.shields.io/crates/v/dm-database-sqllog2db.svg)](https://crates.io/crates/dm-database-sqllog2db)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![GitHub Release](https://img.shields.io/github/v/release/guangl/sqllog2db)](https://github.com/guangl/sqllog2db/releases)
[![Rust 1.56+](https://img.shields.io/badge/Rust-1.56%2B-orange.svg)](https://www.rust-lang.org/)

一个轻量、可扩展的 SQL 日志导出 CLI 工具：解析数据库 SQL 日志（流式），导出到 CSV / JSONL / 数据库（可选），并集中记录解析错误与分类指标。

- 体积小巧：默认功能仅 CSV/JSONL，按需启用数据库导出，最小化二进制体积
- 稳健可靠：流式解析 + 批量导出 + 错误聚合与摘要（errors.summary.json）
- 易于集成：清晰的配置文件（TOML），一条命令跑完

> 适用场景：日志归档、数据分析预处理、基于日志的问责/审计、异构系统导出。

---

## 快速链接

- [Crates.io 包页面](https://crates.io/crates/dm-database-sqllog2db)
- [GitHub 仓库](https://github.com/guangl/sqllog2db)
- [GitHub Releases](https://github.com/guangl/sqllog2db/releases)
- [CHANGELOG](./CHANGELOG.md)

---

## 功能特性

- **流式解析 SQL 日志**：单线程顺序处理，性能优秀且可预测
- **单导出目标**（简化架构）：
  - CSV（默认特性）
  - JSONL（默认特性）
  - SQLite / DuckDB（可选特性）
- **批量导出**：支持按条数进行批量 flush（推荐 10000 条/批）
  - `batch_size > 0`: 每 N 条记录 flush 一次
  - `batch_size = 0`: 累积所有记录，最后一次性 flush
- **错误记录**：
  - 所有解析失败按 JSONL 逐条写入 `errors.jsonl`
  - 生成 `errors.summary.json`，包含总数、分类与子类统计
- **日志系统**：每日滚动、保留天数可配（1-365）
- **二进制体积优化**：feature gating + release 优化配置（LTO、opt-level=z、panic=abort）

---

## 安装与构建

你可以选择多种方式安装或构建。

### 从 crates.io 安装（推荐）

```bash
cargo install dm-database-sqllog2db
```

### 本地构建

**本地构建（开发者推荐）**

```powershell
# 在仓库根目录
cargo build --release
```

**本地安装（把可执行安装到 Cargo bin 目录）**

```powershell
cargo install --path .
```

### 构建数据库导出支持（可选）

启用 SQLite：

```powershell
cargo build --release --features sqlite
```

启用 DuckDB：

```powershell
cargo build --release --features duckdb
```

同时启用：

```powershell
cargo build --release --features "sqlite duckdb"
```

---

## 快速开始

1) 生成默认配置（如已存在可加 `--force` 覆盖）：

```powershell
sqllog2db init -o config.toml --force
```

2) 验证配置：

```powershell
sqllog2db validate -c config.toml
```

3) 运行导出：

```powershell
sqllog2db run -c config.toml
```

---

## 配置文件说明（config.toml）

以下为 `sqllog2db init` 生成的默认模版（可根据需要修改）：

```toml
# SQL 日志导出工具配置文件

[sqllog]
# SQL 日志输入目录（可包含多个日志文件）
directory = "sqllogs"
# 批量提交大小 (0 表示全部解析完成后一次性写入; >0 表示每 N 条记录批量写入)
# 推荐值: 10000 (最佳性能)
batch_size = 10000

[error]
# 解析错误日志输出文件路径（JSON Lines 格式）
file = "errors.jsonl"

[logging]
# 应用日志输出文件路径
file = "logs/sqllog2db.log"
# 日志级别: trace | debug | info | warn | error
level = "info"
# 日志保留天数 (1-365) - 用于滚动文件最大保留数量
retention_days = 7

[features]
# 是否替换 SQL 中的参数占位符（如 ? -> 实际值）
replace_sql_parameters = false
# 是否启用分散导出（按日期或其他维度拆分输出文件）
scatter = false

# ===================== 导出器配置 =====================
# 只支持单个导出器，按优先级选择：CSV > JSONL > Database

# CSV 导出
[exporter.csv]
file = "export/sqllog2db.csv"
overwrite = true

# JSONL 导出（如果同时配置了 CSV，此项将被忽略）
# [exporter.jsonl]
# file = "export/sqllog2db.jsonl"
# overwrite = true

# 数据库导出示例：文件型数据库 (SQLite / DuckDB)
# [exporter.database]
# database_type = "sqlite" # 可选: sqlite | duckdb
# file = "export/sqllog2db.sqlite"
# overwrite = true
# table_name = "sqllog"
```

**配置说明：**
- **字段命名更新（v0.1.2）**：
  - `sqllog.path` → `sqllog.directory` (输入目录)
  - 所有 `path` 字段 → `file` (输出文件)
  - 旧字段名仍然兼容，但建议使用新名称
- 只支持单个导出器，如配置多个将按优先级选择第一个
- `batch_size = 10000` 提供最佳性能
- `logging.retention_days` 必须在 1-365 之间

> **注意**：从 v0.1.1 开始，配置格式已从数组格式 `[[exporter.csv]]` 改为单个导出器格式 `[exporter.csv]`。

## 导出与错误日志

- 导出统计：每个导出器会输出成功/失败条数与（若适用）批量 flush 次数
- 错误日志：
  - `errors.jsonl` 用于记录逐条解析失败的详细信息（JSON Lines）
  - `errors.summary.json` 是自动生成的摘要文件，包含：
    - `total`: 错误总数
    - `by_category`: 各错误大类计数（如 Config/File/Database/Parse/Export 等）
    - `parse_variants`: 解析错误子类型计数（如 InvalidSql/InvalidTimestamp 等）

> 如果没有错误发生，`errors.summary.json` 依然会生成（空计数），便于自动化汇总。

---

## 功能开关与编译特性（features）

- 默认启用：`csv`
- 可选启用：`jsonl`, `sqlite`, `duckdb`

示例：

```powershell
# 默认（CSV）
cargo build --release

# 启用 SQLite
echo "启用 SQLite 特性"
cargo build --release --features sqlite

# 启用 DuckDB
echo "启用 DuckDB 特性"
cargo build --release --features duckdb

# 同时启用
echo "启用 SQLite 和 DuckDB"
cargo build --release --features "sqlite duckdb"
```

> 提示：如果仅使用文件导出（CSV），不启用数据库相关特性可显著减小二进制体积。

---

## 开发与测试

- 运行全部测试：

```powershell
cargo test
```

- 按特性运行（示例：带 DuckDB）

```powershell
cargo test --features duckdb
```

- 常见告警（开发期）：未使用代码/变量（dead_code/unused_variables）不会影响功能，可在重构时清理或加 `#[allow]` 局部抑制。

---

## 性能与体积

### 性能测试结果

**测试环境**: ~1.1GB SQL 日志文件，约 320 万条记录（单线程模式）

| 配置 | 平均用时 | 吞吐量 | 相对性能 |
|------|---------|--------|---------|
| **batch_size=10000 (推荐)** | **8.11s** | **~397K 条/秒** | 100% (最快) |
| batch_size=50000 | 8.43s | ~382K 条/秒 | 104% |
| batch_size=1000 | 8.67s | ~371K 条/秒 | 107% |
| batch_size=0 (全部累积) | 9.15s | ~352K 条/秒 | 113% |

**结论**: 默认配置 `batch_size=10000` 提供最佳性能，在 I/O 效率和内存占用之间达到最佳平衡。

**性能瓶颈分析**（NVMe SSD 测试）：
- 解析：5.62s (69%) - 主要瓶颈
- CSV 格式化：1.51s (19%)
- 文件写入：0.22s (3%)
- 其他开销：0.76s (9%)

运行性能测试：
```bash
cargo bench --bench performance
```

### 二进制体积

- Release 构建已启用：`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`
- 建议仅启用所需特性以获得更小二进制体积
- 单导出器模式移除了多线程开销（已移除 `crossbeam`、`rayon` 依赖）

---

## 故障排查

- **程序无法启动 / 配置解析失败**：
  - 使用 `sqllog2db validate -c config.toml` 检查配置
  - 确保使用新的字段名称（v0.1.2+）：`directory` 和 `file` 而非 `path`
  - 确保 `logging.file` 为合法的文件路径，其父目录可创建
- **未生成导出文件**：
  - 确认 `sqllog.directory` 下是否存在 `.log` 文件
  - 查看应用日志与 `errors.jsonl` 定位问题
  - 检查是否配置了导出器（至少配置一个：CSV/JSONL/Database）
- **数据库导出失败**：
  - 检查 `database_type` 与对应字段（文件型使用 `file`）
  - 确保编译时已启用对应特性（`sqlite` 或 `duckdb`）
- **配置迁移问题**：
  - v0.1.2 更新了字段命名，但保持向后兼容
  - 旧配置文件仍可使用，但建议更新到新字段名

---

## 许可证

本项目建议使用 MIT 或 Apache-2.0 作为开源许可证（可在 `Cargo.toml` 中加入 `license` 或 `license-file` 字段，并在仓库根目录添加许可文件）。如需我直接添加，请告知选择。

---

## 致谢

- 依赖：`clap`、`tracing`、`serde`、`serde_json`、`rusqlite`/`duckdb`（特性可选）等
- 日志/错误与导出架构参考了业内通用实践，并针对体积做了减法优化

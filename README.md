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
- [性能测试报告](./docs/PERFORMANCE.md)
- [架构简化说明](./docs/SIMPLIFICATION.md)

---

## 功能特性

- 流式解析 SQL 日志：单线程顺序处理，性能优秀且可预测
- 单导出目标（简化架构）：
  - CSV（默认特性）
  - JSONL（默认特性）
  - SQLite / DuckDB（可选特性）
- 批量导出：支持按条数进行批量 flush（推荐 10000 条/批）
  - `batch_size > 0`: 每 N 条记录 flush 一次
  - `batch_size = 0`: 累积所有记录，最后一次性 flush
- 错误记录：
  - 所有解析失败按 JSONL 逐条写入 `errors.jsonl`
  - 生成 `errors.summary.json`，包含总数、分类与子类统计
- 日志系统：每日滚动、保留天数可配（1-365）
- 二进制体积优化：feature gating + release 优化配置（LTO、opt-level=z、panic=abort）

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

> Windows 路径注意事项：本工具会为 `logging.path` 的父目录自动创建目录；建议将 `logging.path` 设置为“文件路径”，如 `logs/sqllog2db.log`。

---

## 配置文件说明（config.toml）

以下为 `sqllog2db init` 生成的默认模版（可根据需要修改）：

```toml
# SQL 日志导出工具默认配置文件 (请根据需要修改)

[sqllog]
# SQL 日志目录或文件路径
path = "sqllogs"
# 处理线程数 (0 表示自动，根据文件数量与 CPU 核心数决定)
thread_count = 0
# 批量提交大小 (0 表示全部解析完成后一次性写入; >0 表示每 N 条记录批量写入)
batch_size = 0

[error]
# 解析错误日志（JSON Lines 格式）输出路径
path = "errors.jsonl"

[logging]
# 应用日志输出目录或文件路径 (当前版本要求为“文件路径”，例如 logs/sqllog2db.log)
# 如果仅设置为目录（如 "logs"），请确保后续代码逻辑能够自动生成文件；否则请填写完整文件路径
path = "logs/sqllog2db.log"
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
# 至少需要配置一个导出器 (CSV / JSONL / Database)

# CSV 导出（可配置多个）
[[exporter.csv]]
path = "export/sqllog2db.csv"
overwrite = true

# JSONL 导出（可配置多个）
[[exporter.jsonl]]
path = "export/sqllog2db.jsonl"
overwrite = true

# 数据库导出（可配置多个）示例：文件型数据库 (SQLite / DuckDB)
# [[exporter.database]]
# database_type = "sqlite" # 可选: sqlite | duckdb | postgres | oracle | dm
# path = "export/sqllog2db.sqlite" # 文件型数据库使用 path
# overwrite = true
# table_name = "sqllog"
# batch_size = 1000

# 网络型数据库示例 (DM/PostgreSQL/Oracle)
# [[exporter.database]]
# database_type = "dm"
# host = "localhost"
# port = 5236
# username = "SYSDBA"
# password = "SYSDBA"
# overwrite = true
# table_name = "sqllog"
# batch_size = 1000
# database = "TEST"          # 可选 (postgres/dm)
# service_name = "ORCL"       # Oracle 可选（与 sid 二选一）
# sid = "ORCLSID"             # Oracle 可选（与 service_name 二选一）
```

要点：
- 至少配置一个导出器（CSV/JSONL/Database 任一）
- `logging.retention_days` 必须在 1-365 之间
- `sqllog.thread_count` 为 0 时自动；最多建议 ≤ 256
- 数据库导出需要根据 `database_type` 与实际环境补充必要字段

---

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

- 默认启用：`csv`, `jsonl`
- 可选启用：`sqlite`, `duckdb`

示例：

```powershell
# 默认（CSV+JSONL）
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

> 提示：如果仅使用文件导出（CSV/JSONL），不启用数据库相关特性可显著减小二进制体积。

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

**测试环境**: ~1.1GB SQL 日志文件，约 320 万条记录

| 配置 | 平均用时 | 吞吐量 | 相对性能 |
|------|---------|--------|---------|
| **batch_size=10000 (推荐)** | **8.88s** | **~362K 条/秒** | 100% (最快) |
| batch_size=50000 | 9.24s | ~348K 条/秒 | 104% |
| batch_size=1000 | 9.34s | ~344K 条/秒 | 105% |
| batch_size=0 (全部累积) | 9.64s | ~334K 条/秒 | 108% |

**结论**: 默认配置 `batch_size=10000` 提供最佳性能，在 I/O 效率和内存占用之间达到最佳平衡。

详细性能分析请参考：[性能测试报告](docs/PERFORMANCE.md)

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

- 程序无法启动 / 配置解析失败：
  - 使用 `sqllog2db validate -c config.toml` 检查配置
  - 确保 `logging.path` 为合法的“文件路径”，其父目录可创建
- 未生成导出文件：
  - 确认日志目录下是否存在 `.log` 文件
  - 查看应用日志与 `errors.jsonl` 定位问题
- 数据库导出失败：
  - 检查 `database_type` 与对应字段（文件型使用 `path`；网络型使用 host/port/用户名/密码/可选 database 等）
  - 确保编译时已启用对应特性（`sqlite` 或 `duckdb`）

---

## 许可证

本项目建议使用 MIT 或 Apache-2.0 作为开源许可证（可在 `Cargo.toml` 中加入 `license` 或 `license-file` 字段，并在仓库根目录添加许可文件）。如需我直接添加，请告知选择。

---

## 致谢

- 依赖：`clap`、`tracing`、`serde`、`serde_json`、`rayon`（并行）、`rusqlite`/`duckdb`（特性可选）等
- 日志/错误与导出架构参考了业内通用实践，并针对体积做了减法优化

# 🚀 sqllog2db

[![Crates.io](https://img.shields.io/crates/v/dm-database-sqllog2db?style=flat-square&logo=rust&logoColor=white&label=crates.io&color=d96109)](https://crates.io/crates/dm-database-sqllog2db)
[![Downloads](https://img.shields.io/crates/d/dm-database-sqllog2db?style=flat-square&label=downloads&color=informational)](https://crates.io/crates/dm-database-sqllog2db)
[![CI](https://img.shields.io/github/actions/workflow/status/guangl/sqllog2db/ci.yaml?style=flat-square&logo=github-actions&logoColor=white&label=ci)](https://github.com/guangl/sqllog2db/actions/workflows/ci.yaml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square&logo=apache&logoColor=white)](https://opensource.org/licenses/Apache-2.0)
[![Release](https://img.shields.io/github/v/release/guangl/sqllog2db?style=flat-square&logo=github&logoColor=white&label=release)](https://github.com/guangl/sqllog2db/releases)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)

一个轻量、高效的 SQL 日志导出 CLI 工具：解析达梦数据库 SQL 日志（流式处理），导出到 CSV / Parquet / JSONL / SQLite / DuckDB / PostgreSQL / DM，并提供按行落盘的错误追踪。

- **高性能**：单线程流式处理，~150万条/秒吞吐量（极致优化）
- **稳健可靠**：批量导出 + 解析错误逐行落盘（便于追踪原始日志）
- **易于使用**：清晰的 TOML 配置，三步完成导出任务
- **体积优化**：默认仅 CSV 导出，可选启用其它导出器特性

> 适用场景：日志归档、数据分析预处理、基于日志的问责/审计、异构系统导出。

---

## 快速链接

- [Crates.io 包页面](https://crates.io/crates/dm-database-sqllog2db)
- [GitHub 仓库](https://github.com/guangl/sqllog2db)
- [GitHub Releases](https://github.com/guangl/sqllog2db/releases)
- [CHANGELOG](./CHANGELOG.md)
- [Quickstart](./docs/quickstart.md)
- [Architecture](./docs/architecture.md)
- [Contributing](./CONTRIBUTING.md)
- [Security Policy](./SECURITY.md)

---

## 功能特性

- **流式解析 SQL 日志**：单线程顺序处理，性能可预测（~150万条/秒）
- **单导出目标（按优先级选择）**：csv > parquet > jsonl > sqlite > duckdb > postgres > dm
  - CSV（默认特性，16MB 缓冲优化）
  - Parquet（可选特性，行组/内存优化，支持 `row_group_size` 与 `use_dictionary`）
  - JSONL（可选特性，轻量流式）
  - SQLite / DuckDB / PostgreSQL / DM（可选特性）
- **交互式 TUI 模式**（可选）：使用 `--tui` 标志启用实时进度条和统计界面
- **错误追踪**：解析失败逐条写入配置的错误日志文件（纯文本行，`文件|错误|原始片段|行号`），便于后续 grep/统计
- **日志管理**：每日滚动、保留天数可配（1-365 天）
- **二进制优化**：LTO + strip + panic=abort，体积最小化

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

# 启用 TUI 模式
cargo build --release --features tui
```

**本地安装（把可执行安装到 Cargo bin 目录）**

```powershell
cargo install --path .

# 或安装带 TUI 支持
cargo install --path . --features tui
```

### 构建可选导出器（特性开关）

```powershell
# 默认仅 CSV
cargo build --release

# 选择性启用
cargo build --release --features parquet
cargo build --release --features jsonl
cargo build --release --features sqlite
cargo build --release --features duckdb
cargo build --release --features postgres
cargo build --release --features dm

# 启用多个
cargo build --release --features "parquet jsonl sqlite"

# 组合多个特性（包含 TUI）
cargo build --release --features "csv,tui,parquet,jsonl"
```

> 💡 提示：默认仅包含 CSV 导出，如需其他导出器请按需启用对应 feature。TUI 模式通过 `tui` feature 启用，不影响默认 CLI 模式。

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

3) 运行导出（CLI 模式）：

```powershell
sqllog2db run -c config.toml
```

### TUI 模式（可选）

使用交互式终端 UI 运行导出（需要编译时启用 `tui` feature）：

```bash
# 构建时启用 TUI
cargo build --release --features "csv,tui"

# 运行 TUI 模式
sqllog2db run -c config.toml --tui
```

TUI 模式提供实时进度条、统计信息和交互式界面，按 `q` 或 `Esc` 退出。

### Shell 补全

生成 shell 自动补全脚本：

```bash
# Bash
sqllog2db completions bash > /etc/bash_completion.d/sqllog2db

# Zsh
sqllog2db completions zsh > ~/.zfunc/_sqllog2db

# Fish
sqllog2db completions fish > ~/.config/fish/completions/sqllog2db.fish
```

---

## 配置文件说明（config.toml）

以下为 `sqllog2db init` 生成的默认模版，可根据需要修改：

```toml
# SQL 日志导出工具默认配置文件 (请根据需要修改)

[sqllog]
# SQL 日志目录或文件路径
directory = "sqllogs"

[error]
# 解析错误日志输出路径（内容为纯文本行: file | error | raw | line）
file = "export/errors.log"

[logging]
# 应用日志输出目录或文件路径 (当前版本要求为"文件路径"，例如 logs/sqllog2db.log)
# 如果仅设置为目录（如 "logs"），请确保后续代码逻辑能够自动生成文件；否则请填写完整文件路径
file = "logs/sqllog2db.log"
# 日志级别: trace | debug | info | warn | error
level = "info"
# 日志保留天数 (1-365) - 用于滚动文件最大保留数量
retention_days = 7

[features.replace_parameters]
enable = false
symbols = ["?", ":name", "$1"] # 可选参数占位符样式列表

# ===================== 导出器配置 =====================
# 只能配置一个导出器
# 同时配置多个时，按优先级使用：csv > parquet > jsonl > sqlite > duckdb > postgres > dm

# 方案 1: csv 导出（默认）
[exporter.csv]
file = "outputs/sqllog.csv"
overwrite = true
append = false

# 方案 2: Parquet 导出（使用时注释掉上面的导出器,启用下面的 Parquet）
# [exporter.parquet]
# file = "export/sqllog2db.parquet"
# overwrite = true
# row_group_size = 100000           # 每个 row group 的行数 (默认值)
# use_dictionary = true             # 是否启用字典编码

# 方案 3: JSONL 导出（JSON Lines 格式，每行一个 JSON 对象）
# [exporter.jsonl]
# file = "export/sqllog2db.jsonl"
# overwrite = true
# append = false

# 方案 4: SQLite 数据库导出
# [exporter.sqlite]
# database_url = "export/sqllog2db.db"
# table_name = "sqllog_records"
# overwrite = true
# append = false

# 方案 5: DuckDB 数据库导出（分析型数据库，高性能）
# [exporter.duckdb]
# database_url = "export/sqllog2db.duckdb"
# table_name = "sqllog_records"
# overwrite = true
# append = false

# 方案 6: PostgreSQL 数据库导出
# [exporter.postgres]
# host = "localhost"
# port = 5432
# username = "postgres"
# password = "postgres"
# database = "sqllog"
# schema = "public"
# table_name = "sqllog_records"
# overwrite = true
# append = false

# 方案 7: DM 数据库导出（使用 dmfldr 命令行工具）
# [exporter.dm]
# userid = "SYSDBA/DMDBA_hust4400@localhost:5236"
# table_name = "sqllog_records"
# control_file = "export/sqllog.ctl"
# log_dir = "export/log"
# overwrite = true
# charset = "UTF-8"
```

**配置说明：**
- 只支持单个导出器，如配置多个按优先级选择第一个
- `logging.retention_days` 必须在 1-365 之间
- 默认仅启用 CSV，其他导出器需在编译期开启对应 feature

## 导出与错误日志

- **导出统计**：导出器会输出成功/失败条数与批量 flush 次数
- **错误日志**：由 `[error].file` 指定的文件按行追加记录，格式为 `文件路径 | 错误原因 | 原始内容(换行被转义) | 行号`。当前版本不会额外生成 summary 文件，统计信息会在控制台日志中输出。

---

## 功能特性开关

- **默认启用**：`csv`
- **可选导出器**：`parquet`、`jsonl`、`sqlite`、`duckdb`、`postgres`、`dm`
- **可选功能**：`replace_parameters`（SQL 参数占位符替换）

编译示例：

```powershell
# 默认构建（仅 CSV）
cargo build --release

# 按需启用导出器
cargo build --release --features parquet
cargo build --release --features "jsonl sqlite"
cargo build --release --features "duckdb postgres"
cargo build --release --features dm

# 启用参数替换功能
cargo build --release --features replace_parameters
```

> 💡 **体积优化提示**：只启用必要的导出器特性，可以让二进制更小。

---

## 高级用法

### 日志级别控制

使用全局选项覆盖配置文件中的日志级别：

```bash
# 详细输出（debug 级别）
sqllog2db -v run -c config.toml

# 静默模式（仅错误）
sqllog2db -q run -c config.toml
```

## 开发与测试

运行全部测试：

```powershell
cargo test
```

运行带 SQLite 特性的测试：

```powershell
cargo test --features sqlite
```

运行性能基准测试：

```powershell
cargo bench
```

---

## 性能与体积

### 性能测试结果

**测试环境**: ~1.1GB SQL 日志文件，约 300 万条记录（单线程模式）

| 配置 | 平均用时 | 吞吐量 | 备注 |
|------|---------|--------|------|
| **默认配置 (极致优化)** | **1.94s** | **~1,550K 条/秒** | 零拷贝、缓冲区复用、快速整数转换 |

**性能瓶颈分析**（NVMe SSD 测试）：
- 解析：主要瓶颈
- CSV 格式化：极低开销（已优化）
- 文件写入：极低开销（16MB 缓冲）

运行性能测试：
```bash
cargo bench --bench performance
```

### 二进制体积

- Release 构建已启用：`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`
- 建议仅启用所需特性以获得更小二进制体积
- 单导出器模式移除了多线程开销（已移除 `crossbeam`、`rayon` 依赖）

---

## 常见问题 (FAQ)

**Q: 支持哪些数据库导出格式？**
A: CSV（默认）、Parquet、JSONL、SQLite、DuckDB、PostgreSQL、DM。除 CSV 外其他需编译时启用对应 feature。

**Q: 为什么只支持单个导出器？**
A: 单导出器架构更简单、性能可预测、内存占用低。如需多格式可分多次运行。

**Q: 性能瓶颈在哪里？**
A: 主要在解析（69%），CSV 格式化和写入占比很小。建议使用 NVMe SSD。

**Q: 如何处理超大日志文件？**
A: 工具采用流式处理，内存占用稳定在约 179MB，理论上可处理任意大小文件。

**Q: 错误日志格式是什么？**
A: 纯文本，每行格式为 `文件路径 | 错误原因 | 原始内容 | 行号`，便于 grep 和统计。

**Q: 支持增量导出吗？**
A: 当前版本不支持，需要自行管理已处理文件。未来版本可能添加。

**Q: 如何提高导出速度？**
A: 1) 使用 NVMe SSD；2) 关闭不必要的日志级别（`-q`）；3) 对于 Parquet 调整 `row_group_size`。

---

## 故障排查

- **程序无法启动 / 配置解析失败**：
  - 使用 `sqllog2db validate -c config.toml` 检查配置
  - 确保使用新的字段名称（v0.1.2+）：`directory` 和 `file` 而非 `path`
  - 确保 `logging.file` 为合法的文件路径，其父目录可创建
- **未生成导出文件**：
  - 确认 `sqllog.directory` 下是否存在 `.log` 文件
  - 查看应用日志与 `errors.json` 定位问题
  - 检查是否配置了导出器（至少配置一个：CSV 或 Database）
- **数据库导出失败**：
  - 检查 `database_type` 是否为 `sqlite`
  - 确保编译时已启用 `sqlite` 特性
  - 验证数据库文件路径及父目录可写
- **配置迁移问题**：
  - v0.1.2 更新了字段命名，但保持向后兼容
  - 旧配置文件仍可使用，但建议更新到新字段名

---

## 许可证

本项目采用 Apache-2.0 许可证。详见 [LICENSE](./LICENSE) 文件。

---

## 致谢

核心依赖：
- 日志解析：[dm-database-parser-sqllog](https://crates.io/crates/dm-database-parser-sqllog)
- CLI 框架：[clap](https://crates.io/crates/clap)
- 日志系统：[tracing](https://crates.io/crates/tracing) + [tracing-subscriber](https://crates.io/crates/tracing-subscriber)
- 序列化：[serde](https://crates.io/crates/serde) + [serde_json](https://crates.io/crates/serde_json)
- 数据库（可选）：[rusqlite](https://crates.io/crates/rusqlite)

感谢 Rust 社区提供的优秀生态系统。

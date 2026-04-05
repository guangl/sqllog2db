# 🚀 sqllog2db

[![Crates.io](https://img.shields.io/crates/v/dm-database-sqllog2db?style=flat-square&logo=rust&logoColor=white&label=crates.io&color=d96109)](https://crates.io/crates/dm-database-sqllog2db)
[![Downloads](https://img.shields.io/crates/d/dm-database-sqllog2db?style=flat-square&label=downloads&color=informational)](https://crates.io/crates/dm-database-sqllog2db)
[![CI](https://img.shields.io/github/actions/workflow/status/guangl/sqllog2db/ci.yaml?style=flat-square&logo=github-actions&logoColor=white&label=ci)](https://github.com/guangl/sqllog2db/actions/workflows/ci.yaml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=flat-square&logo=apache&logoColor=white)](https://opensource.org/licenses/Apache-2.0)
[![Release](https://img.shields.io/github/v/release/guangl/sqllog2db?style=flat-square&logo=github&logoColor=white&label=release)](https://github.com/guangl/sqllog2db/releases)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)

一个轻量、高效的 SQL 日志导出 CLI 工具：解析达梦数据库 SQL 日志（流式处理），导出到 CSV / SQLite。

- **高性能**：单线程流式处理，~155万条/秒吞吐量（mmap + SIMD + 零分配优化）
- **输入灵活**：支持单文件、目录（自动扫描 `.log`）或 glob 模式（如 `./logs/*.log`）
- **易于使用**：清晰的 TOML 配置，三步完成导出任务；进度条实时反馈
- **开箱即用**：CSV / SQLite 两种导出器及所有过滤功能均内置，无需额外编译开关

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

- **流式解析 SQL 日志**：单线程顺序处理，性能可预测（~155万条/秒）
- **灵活输入**：单文件、目录扫描（`.log` 文件）、glob 模式（`./logs/2025-*.log`），结果按路径排序
- **单导出目标（按优先级选择）**：csv > sqlite
  - CSV（16MB 缓冲优化，`itoa` 零分配整数格式化）
  - SQLite（批量事务，`PRAGMA` 性能调优）
- **SQL 参数标准化**：自动替换占位符，导出 `normalized_sql` 列，支持 `?` 和 `:N` 两种风格
- **灵活过滤**：记录级（时间范围、用户、IP、标签）与事务级（执行时长、行数、exec_id）过滤
- **统计分析**：`stats` 命令支持每文件明细（`-v`）和最慢查询排行（`--top N`）
- **日志管理**：可配置级别与保留天数（1-365 天）
- **二进制优化**：LTO + strip + panic=abort，体积最小化

---

## 安装与构建

### 从 crates.io 安装（推荐）

```bash
cargo install dm-database-sqllog2db
```

### 本地构建

```bash
# 在仓库根目录
cargo build --release
```

```bash
# 安装到 Cargo bin 目录
cargo install --path .
```

---

## 快速开始

1) 生成默认配置（如已存在可加 `--force` 覆盖）：

```bash
sqllog2db init -o config.toml --force
```

2) 验证配置：

```bash
sqllog2db validate -c config.toml
```

3) 运行导出：

```bash
sqllog2db run -c config.toml
```

### 常用 run 选项

```bash
# 限制最多导出 1000 条（快速抽样）
sqllog2db run -c config.toml --limit 1000

# 只解析不写文件（dry-run）
sqllog2db run -c config.toml --dry-run

# 命令行覆盖配置字段
sqllog2db run -c config.toml --set exporter.csv.file=out.csv

# 按时间范围过滤
sqllog2db run -c config.toml --from "2025-01-01" --to "2025-12-31"

# 静默模式
sqllog2db -q run -c config.toml

# 关闭颜色输出
sqllog2db --no-color run -c config.toml

# 通过环境变量指定配置文件
SQLLOG2DB_CONFIG=config.toml sqllog2db run
```

### 统计日志记录数

无需导出，直接统计日志目录中所有文件的记录数：

```bash
sqllog2db stats -c config.toml
sqllog2db stats --set sqllog.path=./logs

# 显示每文件处理明细
sqllog2db stats -c config.toml -v

# 按执行时长排名前 10 的慢查询
sqllog2db stats -c config.toml --top 10

# 时间范围 + 慢查询排行
sqllog2db stats -c config.toml --from "2025-01-01" --to "2025-12-31" --top 20
```

### 查看当前生效配置

```bash
sqllog2db show-config -c config.toml
sqllog2db show-config -c config.toml --set exporter.csv.file=out.csv
```

### 生成 man page

```bash
sqllog2db man > /usr/local/share/man/man1/sqllog2db.1
```

### Shell 补全

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
# SQL 日志路径：目录、单文件或 glob 模式（如 "./logs/2025-*.log"）
path = "sqllogs"

[logging]
# 应用日志文件路径
file = "logs/sqllog2db.log"
# 日志级别: trace | debug | info | warn | error
level = "info"
# 日志保留天数 (1-365)
retention_days = 7

[features.replace_parameters]
# 是否在导出结果中写入 normalized_sql 列（默认 true）
enable = true

# ===================== 导出器配置 =====================
# 只能配置一个导出器，同时配置多个时按优先级使用：csv > sqlite

# 方案 1: CSV 导出（默认）
[exporter.csv]
file = "outputs/sqllog.csv"
overwrite = true
append = false

# 方案 2: SQLite 数据库导出
# [exporter.sqlite]
# database_url = "export/sqllog2db.db"
# table_name = "sqllog_records"
# overwrite = true
# append = false
```

**配置说明：**
- 只支持单个导出器，如配置多个按优先级选择第一个（csv > sqlite）
- `logging.retention_days` 必须在 1-365 之间
- `sqllog.path` 支持目录、单文件或 glob 模式；旧版字段名 `directory` 仍向后兼容

---

## 导出与日志

- **导出统计**：运行结束后输出成功/失败条数与耗时
- **解析错误**：解析失败的行以 `trace` 级别写入应用日志，不中断处理流程
- **SQL 参数标准化**：`[features.replace_parameters]` 启用时，导出结果含 `normalized_sql` 列（参数值替换为 `?` 或 `:N`）
- **时间范围过滤**：`[features.filters]` 支持 `start_ts`/`end_ts` 毫秒级时间范围
- **SQL Tag 支持**：同步导出 SQL 日志中的 Tag 标签，支持按标签过滤

---

## 高级用法

### 日志级别控制

```bash
# 详细输出（debug 级别）
sqllog2db -v run -c config.toml

# 静默模式（仅错误输出，隐藏进度条和摘要）
sqllog2db -q run -c config.toml
```

### Ctrl+C 优雅退出

运行时按 Ctrl+C，程序会在当前 batch 处理完毕后停止，已处理数据正常写入磁盘，退出码为 130。

### 退出码说明

| 退出码 | 含义 |
|--------|------|
| 0 | 成功 |
| 2 | 配置错误 |
| 3 | 文件/解析错误 |
| 4 | 导出错误 |
| 130 | 用户中断（Ctrl+C） |

---

## 开发与测试

```bash
# 运行全部测试
cargo test

# 代码检查（零警告）
cargo clippy --all-targets -- -D warnings

# 格式化
cargo fmt

# 测试覆盖率（需安装 cargo-llvm-cov）
cargo llvm-cov --summary-only

# 性能基准测试
cargo bench
```

CI 在每次 PR 时自动检查：
- `cargo test`（多平台）
- `cargo clippy --all-targets -- -D warnings`
- `cargo bench --no-run`（确保 bench 可编译）
- `cargo llvm-cov --fail-under-lines 70`（行覆盖率 ≥ 70%）
- `cargo test --release --test integration test_csv_throughput_baseline`（release 模式性能基准 ≥ 500k 条/秒）

---

## 性能与体积

### 基准测试结果（本地 Apple M 系列，50k 条合成数据）

| 导出器 | 吞吐量 | 备注 |
|--------|--------|------|
| CSV | ~2.13M 条/秒 | 16MB 缓冲 + `itoa` 零分配 |
| SQLite | ~1.11M 条/秒 | 单事务批量写入 + `PRAGMA` 调优 |
| Filters（预扫描） | ~2.25M 条/秒 | `rayon` 并行预扫描 |

**真实场景**（~1.1GB，约 300 万条记录，NVMe SSD）：~1.55M 条/秒

运行基准测试：
```bash
cargo bench
```

### 二进制体积

Release 构建已启用：`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`

---

## 常见问题 (FAQ)

**Q: 支持哪些导出格式？**
A: CSV 和 SQLite 两种格式均内置，无需额外编译开关，通过配置文件选择即可。

**Q: 为什么只支持单个导出器？**
A: 单导出器架构更简单、性能可预测、内存占用低。如需多格式可分多次运行。

**Q: 性能瓶颈在哪里？**
A: 主要在解析层，CSV 格式化和写入占比很小。建议使用 NVMe SSD。

**Q: 如何处理超大日志文件？**
A: 工具采用流式处理，内存占用稳定，理论上可处理任意大小文件。

**Q: 支持增量导出吗？**
A: 当前版本不支持，需要自行管理已处理文件。

**Q: 如何提高导出速度？**
A: 1) 使用 NVMe SSD；2) 静默模式（`-q`）减少终端 I/O。

---

## 故障排查

- **程序无法启动 / 配置解析失败**：
  - 使用 `sqllog2db validate -c config.toml` 检查配置
  - 确保 `logging.file` 为合法的文件路径，其父目录可创建
- **未生成导出文件**：
  - 确认 `sqllog.path` 指向的目录/文件是否存在 `.log` 文件
  - 查看应用日志定位问题（解析错误以 `trace` 级别记录）
  - 检查是否配置了导出器（至少配置一个）
- **SQLite 导出失败**：
  - 验证 `database_url` 路径及父目录可写
  - 检查是否有其他进程持有数据库文件锁

---

## 许可证

本项目采用 Apache-2.0 许可证。详见 [LICENSE](./LICENSE) 文件。

---

## 致谢

核心依赖：
- 日志解析：[dm-database-parser-sqllog](https://crates.io/crates/dm-database-parser-sqllog)
- CLI 框架：[clap](https://crates.io/crates/clap)
- 日志系统：[log](https://crates.io/crates/log)
- 序列化：[serde](https://crates.io/crates/serde)
- 数据库：[rusqlite](https://crates.io/crates/rusqlite)

感谢 Rust 社区提供的优秀生态系统。

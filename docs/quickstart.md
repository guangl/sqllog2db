# Quickstart

最短路径上手 sqllog2db：安装、配置、运行，以及可选特性说明。

## 1) 安装

```powershell
# 从 crates.io 安装
cargo install dm-database-sqllog2db
```

## 2) 生成配置

```powershell
sqllog2db init -o config.toml --force
```

关键默认值（与代码保持一致）：
- `sqllog.directory = "sqllogs"`
- `error.file = "export/errors.log"`（按行追加: `file | error | raw | line`）
- 默认导出器：CSV，输出到 `outputs/sqllog.csv`
- 导出器优先级：CSV > Parquet > JSONL > SQLite > DuckDB > PostgreSQL > DM

## 3) 校验配置

```powershell
sqllog2db validate -c config.toml
```

## 4) 执行导出

```powershell
sqllog2db run -c config.toml
```

控制台会输出导出统计；解析失败会被写入 `error.file` 指定的文本文件。

## 可选特性 / 编译示例

仅启用需要的导出器可减小二进制体积：

```powershell
# 默认构建（仅 CSV）
cargo build --release

# 启用单个导出器
cargo build --release --features parquet
cargo build --release --features jsonl
cargo build --release --features sqlite
cargo build --release --features duckdb
cargo build --release --features postgres
cargo build --release --features dm

# 组合导出器
cargo build --release --features "jsonl sqlite"
cargo build --release --features "duckdb postgres"

# 启用参数替换功能
cargo build --release --features replace_parameters
```

## 最小可用配置示例

```toml
[sqllog]
directory = "sqllogs"

[error]
file = "export/errors.log"

[logging]
file = "logs/sqllog2db.log"
level = "info"
retention_days = 7

[exporter.csv]
file = "outputs/sqllog.csv"
overwrite = true
append = false
```

## 测试与验证

```powershell
# 默认特性
cargo test

# 带特性的测试（示例）
cargo test --features "sqlite"
cargo test --features "duckdb postgres"
```

## 故障排查速查

- 没有输出文件：确认 `sqllog.directory` 下有 `.log` 文件且启用了至少一个导出器。
- 错误日志为空：检查 `error.file` 路径是否可写，查看应用日志。
- PostgreSQL/DuckDB/SQLite 连接失败：确认已在编译期启用对应特性，并检查路径/凭据。

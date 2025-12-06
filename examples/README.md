# Examples

本目录包含 sqllog2db 的示例配置文件。

## 可用示例

### 基础导出
- **config-csv.toml**: CSV 文件导出（默认，无需额外特性）
- **config-parquet.toml**: Parquet 列式存储导出（需 `--features parquet`）
- **config-sqlite.toml**: SQLite 数据库导出（需 `--features sqlite`）

### 高级导出
- **config-postgres.toml**: PostgreSQL 数据库导出（需 `--features postgres`）

## 使用方法

1. 复制示例配置到项目根目录：
   ```bash
   cp examples/config-csv.toml config.toml
   ```

2. 根据需要修改配置（路径、参数等）

3. 运行导出：
   ```bash
   # CSV（默认构建）
   sqllog2db run -c config.toml

   # Parquet（需重新编译）
   cargo build --release --features parquet
   sqllog2db run -c config.toml

   # SQLite（需重新编译）
   cargo build --release --features sqlite
   sqllog2db run -c config.toml
   ```

## 测试配置

验证配置文件是否正确：

```bash
sqllog2db validate -c examples/config-csv.toml
```

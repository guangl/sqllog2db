# 数据库导出功能使用指南

本项目支持将 SQL 日志导出到多种数据库系统。

## 支持的数据库

### 1. SQLite
轻量级嵌入式数据库,无需额外安装。

**编译:**
```bash
cargo build --release --features sqlite
```

**配置示例:**
```toml
[exporter.sqlite]
database_url = "export/sqllog2db.db"
```

**运行:**
```bash
cargo run --release --features sqlite -- run -c config.sqlite.toml
```

**查询数据:**
```bash
sqlite3 export/sqllog2db.db
```
```sql
SELECT COUNT(*) FROM sqllog;
SELECT * FROM sqllog LIMIT 10;
SELECT username, COUNT(*) as cnt FROM sqllog GROUP BY username ORDER BY cnt DESC;
```

### 2. DuckDB
高性能分析型数据库,适合大数据分析。

**编译:**
```bash
cargo build --release --features duckdb
```

**配置示例:**
```toml
[exporter.duckdb]
database_url = "export/sqllog2db.duckdb"
```

**运行:**
```bash
cargo run --release --features duckdb -- run -c config.duckdb.toml
```

**查询数据:**
```bash
duckdb export/sqllog2db.duckdb
```
```sql
SELECT COUNT(*) FROM sqllog;
SELECT username, COUNT(*) as cnt FROM sqllog GROUP BY username ORDER BY cnt DESC LIMIT 10;
-- DuckDB 支持复杂分析查询
SELECT 
    DATE_TRUNC('hour', CAST(ts AS TIMESTAMP)) as hour,
    COUNT(*) as query_count,
    AVG(exec_time_ms) as avg_exec_time
FROM sqllog 
GROUP BY hour 
ORDER BY hour;
```

### 3. PostgreSQL
企业级关系数据库,支持高并发。

**前置条件:**
- 安装并启动 PostgreSQL 服务器
- 创建数据库: `createdb sqllog`

**编译:**
```bash
cargo build --release --features postgres
```

**配置示例:**
```toml
[exporter.postgres]
connection_string = "host=localhost user=postgres password=your_password dbname=sqllog"
```

**运行:**
```bash
cargo run --release --features postgres -- run -c config.postgres.toml
```

**查询数据:**
```bash
psql -d sqllog
```
```sql
SELECT COUNT(*) FROM sqllog;
SELECT * FROM sqllog LIMIT 10;
-- 创建索引优化查询
CREATE INDEX idx_username ON sqllog(username);
CREATE INDEX idx_ts ON sqllog(ts);
-- 复杂分析
SELECT 
    username,
    COUNT(*) as total_queries,
    AVG(exec_time_ms) as avg_time,
    MAX(exec_time_ms) as max_time
FROM sqllog 
WHERE exec_time_ms IS NOT NULL
GROUP BY username
ORDER BY avg_time DESC;
```

## 数据库表结构

所有数据库使用相同的表结构:

```sql
CREATE TABLE sqllog (
    id INTEGER/SERIAL PRIMARY KEY,  -- 自增主键
    ts VARCHAR NOT NULL,              -- 时间戳
    ep INTEGER NOT NULL,              -- 端点
    sess_id VARCHAR NOT NULL,         -- 会话 ID
    thrd_id VARCHAR NOT NULL,         -- 线程 ID
    username VARCHAR NOT NULL,        -- 用户名
    trx_id VARCHAR NOT NULL,          -- 事务 ID
    statement VARCHAR NOT NULL,       -- 语句类型
    appname VARCHAR NOT NULL,         -- 应用名称
    client_ip VARCHAR NOT NULL,       -- 客户端 IP
    sql TEXT NOT NULL,                -- SQL 语句
    exec_time_ms REAL/FLOAT,          -- 执行时间(毫秒)
    row_count INTEGER,                -- 影响行数
    exec_id BIGINT                    -- 执行 ID
);
```

## 性能对比

| 数据库 | 插入速度 | 查询速度 | 文件大小 | 适用场景 |
|--------|----------|----------|----------|----------|
| SQLite | 中 | 中 | 小 | 单机、小数据量 |
| DuckDB | 快 | 非常快 | 中 | 分析查询、OLAP |
| PostgreSQL | 快 | 快 | 大 | 生产环境、高并发 |

## 批量大小调优

通过配置 `batch_size` 优化性能:

```toml
[sqllog]
batch_size = 10000  # SQLite/DuckDB 推荐 5000-10000
                    # PostgreSQL 可以更大,如 50000
```

## 组合使用

可以同时启用多个导出器功能:

```bash
cargo build --release --features "sqlite,duckdb,postgres"
```

然后在配置文件中选择使用哪个(按优先级)。

## 故障排查

### SQLite
- 确保目标目录存在且有写权限
- 检查磁盘空间

### DuckDB  
- 确保安装了 DuckDB CLI (可选,用于查询)
- 文件格式与版本兼容

### PostgreSQL
- 检查连接字符串是否正确
- 确保 PostgreSQL 服务运行中
- 检查用户权限: `GRANT ALL ON DATABASE sqllog TO your_user;`
- 检查防火墙/网络连接

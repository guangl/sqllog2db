# Architecture

sqllog2db 的架构设计文档。

## 总体架构

```
┌─────────────┐
│  CLI Layer  │  命令行接口 (clap)
└──────┬──────┘
       │
┌──────▼──────┐
│   Config    │  配置管理 (TOML)
└──────┬──────┘
       │
┌──────▼──────────────────────┐
│  Parser (dm-database-parser)│  SQL 日志解析
└──────┬──────────────────────┘
       │
┌──────▼──────────┐
│ Exporter Manager│  导出器管理（单导出器架构）
└──────┬──────────┘
       │
┌──────▼─────────────────────────────────────┐
│  Exporters (CSV/Parquet/JSONL/SQLite/...)  │
└────────────────────────────────────────────┘
```

## 核心模块

### 1. CLI 层 (`src/cli/`)
- **opts.rs**: 命令行参数定义 (clap)
  - `init`: 生成配置文件
  - `validate`: 验证配置
  - `run`: 执行导出任务
  - `completions`: 生成 shell 补全脚本
- **init.rs**: 配置文件生成
- **validate.rs**: 配置验证
- **run.rs**: 主流程协调

### 2. 配置管理 (`src/config.rs`)
- TOML 格式配置解析
- 配置验证逻辑
- 默认值定义
- 支持特性条件编译 (`#[cfg(feature = "...")]`)

关键配置结构：
- `SqllogConfig`: 输入日志路径
- `ErrorConfig`: 错误日志输出
- `LoggingConfig`: 应用日志配置
- `ExporterConfig`: 导出器配置（单选）
- `FeaturesConfig`: 功能开关（如参数替换）

### 3. 解析器 (`src/parser.rs`)
- 封装 `dm-database-parser-sqllog` 库
- 流式解析：单线程顺序处理
- 内存优化：批次大小 1000 条

工作流程：
1. 扫描输入目录，收集 `.log` 文件
2. 逐文件顺序解析
3. 批量累积记录（1000 条）
4. 批量传递给导出器

### 4. 导出器架构 (`src/exporter/`)

#### 单导出器设计原则
- **简化架构**：移除多导出器并发支持（v0.1.1+）
- **可预测性能**：单线程顺序处理，避免线程竞争
- **内存友好**：批处理 + 及时释放

#### Exporter Trait
```rust
pub trait Exporter {
    fn initialize(&mut self) -> Result<()>;
    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()>;
    fn export_batch(&mut self, sqllogs: &[&Sqllog<'_>]) -> Result<()>;
    fn finalize(&mut self) -> Result<()>;
    fn name(&self) -> &str;
    fn stats_snapshot(&self) -> Option<ExportStats>;
}
```

#### 导出器优先级
当配置多个导出器时，按以下顺序选择第一个：
1. CSV
2. Parquet
3. JSONL
4. SQLite
5. DuckDB
6. PostgreSQL
7. DM

#### 已实现导出器
- **CSV** (`csv.rs`): 16MB 缓冲，零拷贝优化
- **Parquet** (`parquet.rs`): Arrow + 行组优化
- **JSONL** (`jsonl.rs`): 轻量流式
- **SQLite** (`sqlite.rs`): CSV 中间转换
- **DuckDB** (`duckdb.rs`): CSV 批量导入
- **PostgreSQL** (`postgres.rs`): COPY 协议
- **DM** (`dm.rs`): dmfldr 工具封装

### 5. 错误处理 (`src/error.rs`, `src/error_logger.rs`)

#### 错误分类
- `ConfigError`: 配置相关
- `FileError`: 文件操作
- `ParserError`: 解析失败
- `DatabaseError`: 数据库连接/操作
- `ExportError`: 导出失败

#### 错误记录
- 解析失败逐行追加到 `error.file`
- 格式：`file | error | raw | line`
- 统计信息在日志中输出

### 6. 日志系统 (`src/logging.rs`)
- 基于 `tracing` + `tracing-subscriber`
- 每日滚动（`appender::rolling::daily`）
- 可配置保留天数（1-365）
- 支持 `--verbose`/`--quiet` 全局选项

## 数据流

```
SQL Log Files
    │
    ▼
LogParser::from_path  ──┐
    │                   │ (per file)
    ▼                   │
Iterator<Sqllog>  ◄─────┘
    │
    ▼
Batch (1000 records)
    │
    ▼
Exporter::export_batch
    │
    ├─► CSV Writer (16MB buffer)
    ├─► Parquet Writer (row groups)
    ├─► JSONL Writer
    ├─► SQLite (CSV import)
    ├─► DuckDB (CSV import)
    ├─► PostgreSQL (COPY)
    └─► DM (dmfldr)
    │
    ▼
Output File/Database
```

## 性能优化

### 已实施
1. **零拷贝写入**（CSV）：直接写入缓冲区
2. **批处理**：1000 条/批，降低 I/O 次数
3. **缓冲区优化**：CSV 16MB，Parquet 行组大小可配
4. **单线程架构**：避免线程切换开销
5. **惰性求值**：流式处理，避免全量加载

### 性能剖析（NVMe SSD，1.1GB 数据）
- 解析：69%（主要瓶颈）
- CSV 格式化：19%
- 文件写入：3%
- 其他：9%

吞吐量：~150 万条/秒

## 特性开关 (Feature Flags)

```toml
[features]
default = ["csv"]

# 导出器
csv = ["dep:csv"]
parquet = ["dep:parquet", "dep:arrow"]
jsonl = ["dep:serde_json"]
sqlite = ["dep:rusqlite", "dep:tempfile", "csv"]
duckdb = ["dep:duckdb", "dep:tempfile", "csv"]
postgres = ["dep:postgres", "dep:tempfile", "csv"]
dm = ["dep:tempfile", "csv"]

# 功能
replace_parameters = ["dep:anyhow"]
```

**依赖关系**：
- SQLite/DuckDB/PostgreSQL/DM 依赖 CSV（中间格式）
- Parquet 依赖 Arrow

## 设计决策

### 为什么单导出器？
- **简化复杂度**：多导出器并发收益有限
- **可预测性**：单线程性能更稳定
- **内存友好**：避免多缓冲区同时占用
- **易于维护**：代码更简洁

### 为什么使用 CSV 作为中间格式？
- DM/DuckDB 原生支持 CSV 批量导入
- PostgreSQL COPY 协议高效
- SQLite CSV 虚拟表性能优秀
- 实现简单，兼容性好

### 为什么批次大小是 1000？
- 平衡内存占用与 I/O 效率
- 适合大多数场景
- 及时释放内存，避免峰值过高

## 未来改进方向

- [ ] 进度条（大文件处理）
- [ ] `--dry-run` 模式
- [ ] 并行文件解析（可选）
- [ ] 更多导出器（MySQL、Oracle 直连等）
- [ ] 增量导出支持
- [ ] 错误自动摘要生成

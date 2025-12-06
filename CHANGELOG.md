# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-12-06

### Added
- **Shell 自动补全**：新增 `completions` 子命令，支持生成 Bash/Zsh/Fish 补全脚本
- **全局日志级别选项**：`-v/--verbose` 和 `-q/--quiet` 覆盖配置文件日志级别
- **示例配置**：`examples/` 目录包含 CSV/Parquet/SQLite/PostgreSQL 示例配置
- **开发者文档**：
  - `CONTRIBUTING.md`：贡献指南
  - `docs/architecture.md`：架构设计文档
  - `docs/quickstart.md`：快速开始指南
- **安全策略**：`SECURITY.md` 安全漏洞报告流程
- **CI/CD 增强**：
  - `.github/workflows/ci.yaml`：自动化测试、Clippy、格式检查、文档检查
  - `.github/workflows/publish.yaml`：自动发布到 crates.io
  - `.github/dependabot.yml`：自动依赖更新
- **代码规范**：`rustfmt.toml` 和 `.editorconfig` 统一代码风格
- **README 徽章**：CI status 和 downloads 徽章

### Changed
- **Cargo.toml**：添加 `authors` 字段，增加 `clap_complete` 依赖
- **CLI 描述优化**：更详细的 `about` 和 `long_about` 说明
- 默认配置模板与代码默认值对齐：`sqllog.directory = "sqllogs"`、错误日志默认输出到 `export/errors.log`（按行文本）
- README 配置示例与导出器优先级、默认路径保持一致
- 导出器优先级警告信息更详细（包含完整优先级列表）

### Removed
- 移除未使用的 `dashmap` 依赖

### Fixed
- `oracle.rs` 中用 `ok_or_else` 替换 `unwrap()`，提高错误处理健壮性

### Performance
- 内存优化：总峰值内存从 2.42GB 降至约 179MB（-92.6%），Parquet 峰值从 2.37GB 降至 ~134MB
- 批处理与块处理参数调整（500/1000）保持或提升导出性能


## [0.1.2] - 2025-11-13

### Changed
- **配置字段命名统一**: 将 `path` 字段改为更明确的命名以区分目录和文件
  - `sqllog.path` → `sqllog.directory` (输入目录)
  - `error.path` → `error.file` (输出文件)
  - `logging.path` → `logging.file` (输出文件)
  - `exporter.csv.path` → `exporter.csv.file` (输出文件)
  - `exporter.jsonl.path` → `exporter.jsonl.file` (输出文件)
  - `exporter.database.path` → `exporter.database.file` (输出文件)
- 旧的 `path` 字段名通过 `serde(alias)` 保持向后兼容，无需立即修改现有配置文件

### Removed
- 移除过时的性能分析文档（PARSER_PERFORMANCE_ANALYSIS.md, PERFORMANCE.md）
- 移除架构简化说明文档（SIMPLIFICATION.md）
- 清理临时测试文件（test.log, test.out, test_output.txt）

### Performance
- **once_cell 优化**：
  - CSV 头部字符串缓存（避免重复构造）
  - 日志级别映射 HashMap 缓存（优化解析性能）

## [0.1.1] - 2025-01-XX

### Added

- **性能分析文档**：新增 `docs/PARSER_PERFORMANCE_ANALYSIS.md`，详细分析解析库性能瓶颈
- **架构简化文档**：新增 `docs/SIMPLIFICATION.md`，说明单导出器架构的设计决策

### Changed

- **简化架构为单导出器模式**：
  - 移除多导出器并发支持，现在只支持配置单个导出器
  - 移除多线程并发解析和导出逻辑，回归单线程顺序处理
  - 移除 `crossbeam` 和 `rayon` 依赖，减小二进制体积
  - 简化代码结构，提高可维护性和可预测性
  - 当配置多个导出器时，按优先级使用第一个：CSV > JSONL > Database
- **配置格式变更**（不向后兼容）：
  - 原 `[[exporter.csv]]` / `[[exporter.jsonl]]` 等数组格式改为单个配置
  - 新格式：`[exporter.csv]` / `[exporter.jsonl]` / `[exporter.database]`（三选一）
- **batch_size 语义变更**：
  - `batch_size = 0` 现在表示"累积所有记录，最后一次性 flush"
  - `batch_size > 0` 表示"每 N 条记录 flush 一次"

### Removed

- 移除 `SqllogParser::new()` 的 `thread_count` 参数（不再支持多线程）
- 移除 `SqllogParser::parse_with()` 方法（改用直接调用 `dm-database-parser-sqllog` 的 API）
- 移除 `ExporterManager::count()` 方法（现在只有单个导出器）
- 移除 `ExporterManager::into_exporters()` 方法
- 移除导出器线程和 channel 通信机制
- 移除 `Arc<Sqllog>` 包装，直接使用引用传递
- 移除 `Exporter` trait 的 `Send` 约束
- 移除 `crossbeam` 和 `rayon` 依赖

### Fixed

- 修复 CSV 导出器测试中基于旧实现的断言
- 修复集成测试缺少 feature 条件编译标记的问题

### Performance

- **CSV 写入优化**（v2.0）：
  - 零拷贝字段写入（直接写入缓冲区）
  - 重用行缓冲区（避免重复分配）
  - 优化转义逻辑
  - 增大文件缓冲区至 8MB
  - 性能提升约 9.5%（8.88s → 8.11s，处理 1.1GB/320万条记录）
- 单线程模式下性能更可预测
- 对于机械硬盘和文件大小不均的场景，性能更稳定
- 减少了线程切换和 channel 通信的开销
- **性能剖析**（NVMe SSD，1.1GB 数据）：
  - 解析：5.62s（69%，主要瓶颈）
  - CSV 格式化：1.51s（19%）
  - 文件写入：0.22s（3%）
  - 其他开销：0.76s（9%）
  - 总计：8.11s
  - 吞吐量：396,928 条/秒，136 MB/秒

## [0.1.0] - 2025-11-09

### Added

- **流式 SQL 日志解析**：支持多文件并行解析，自动线程数调整
- **多导出目标**：
  - CSV 导出器（默认特性）
  - JSONL 导出器（默认特性）
  - SQLite 导出器（可选特性 `sqlite`）
  - DuckDB 导出器（可选特性 `duckdb`）
- **批量导出**：支持配置批量提交大小（`batch_size`），优化内存与吞吐
- **错误聚合与分类**：
  - 解析失败逐条记录到 `errors.jsonl`（JSON Lines 格式）
  - 自动生成 `errors.summary.txt`，包含错误总数、分类统计、解析错误子类型分布
- **日志系统**：
  - 每日滚动日志文件
  - 可配置日志级别（trace/debug/info/warn/error）
  - 可配置保留天数（1-365）
- **CLI 命令**：
  - `init`：生成默认配置文件
  - `validate`：验证配置文件
  - `run`：执行日志导出任务
- **配置管理**：
  - TOML 配置格式
  - 支持多导出器并行配置
  - 数据库导出支持文件型（SQLite/DuckDB）与网络型（PostgreSQL/Oracle/DM）
- **Feature flags**：默认启用 CSV+JSONL，可选启用数据库导出，减小二进制体积
- **Release 优化**：
  - LTO 优化、opt-level=z、strip、panic=abort
  - 多平台预编译二进制（x86_64 Linux/aarch64 Linux/x86_64 Windows）
  - 自动 SHA256 校验文件生成

### Documentation

- 完整的 README.md，涵盖安装、使用、配置、故障排查
- 配置文件详细注释与示例
- GitHub Actions 自动发布流程

### CI/CD

- GitHub Actions 工作流：tag 触发自动构建与发布
- 多平台交叉编译支持
- Release artifacts 附带 SHA256 校验文件

[0.1.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.1.0

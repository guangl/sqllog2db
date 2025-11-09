# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
  - 自动生成 `errors.summary.json`，包含错误总数、分类统计、解析错误子类型分布
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

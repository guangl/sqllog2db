# Contributing to sqllog2db

感谢你对 sqllog2db 的关注！我们欢迎各种形式的贡献。

## 开发环境设置

### 前置要求
- Rust 1.78+ (edition 2024)
- Git

### 克隆仓库
```bash
git clone https://github.com/guangl/sqllog2db.git
cd sqllog2db
```

### 编译
```bash
# 默认特性（仅 CSV）
cargo build --release

# 所有特性
cargo build --release --all-features
```

## 开发流程

### 1. 创建功能分支
```bash
git checkout -b feature/your-feature-name
```

### 2. 进行开发
- 编写代码
- 添加测试
- 更新文档

### 3. 运行测试
```bash
# 默认特性
cargo test

# 所有特性
cargo test --all-features

# 特定导出器
cargo test --features "sqlite duckdb"
```

### 4. 代码检查
```bash
# 格式化
cargo fmt --all

# Clippy 检查
cargo clippy --all-targets --all-features -- -D warnings

# 文档检查
cargo doc --all-features --no-deps
```

### 5. 提交变更
遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```bash
# 功能
git commit -m "feat: add new exporter for MySQL"

# 修复
git commit -m "fix: resolve parsing error for edge case"

# 文档
git commit -m "docs: update quickstart guide"

# 性能
git commit -m "perf: optimize CSV buffer size"

# 重构
git commit -m "refactor: simplify error handling"
```

### 6. 创建 Pull Request
- 推送分支到 GitHub
- 创建 PR，描述你的更改
- 确保 CI 通过

## 代码风格

- 遵循 Rust 标准编码规范
- 运行 `cargo fmt` 格式化代码
- 运行 `cargo clippy` 消除警告
- 为公共 API 添加文档注释

## 添加新导出器

如果你想添加新的导出器，参考现有导出器的实现：

1. 在 `src/exporter/` 下创建新文件（如 `mysql.rs`）
2. 实现 `Exporter` trait
3. 在 `Cargo.toml` 添加可选依赖和 feature
4. 在 `src/exporter/mod.rs` 注册新导出器
5. 更新 `ExporterManager::from_config` 优先级
6. 更新文档和示例配置

## 添加测试

- 单元测试：在对应模块的 `tests` 子模块
- 集成测试：在 `tests/` 目录
- 使用 `#[cfg(test)]` 条件编译
- 为可选特性使用 `#[cfg(feature = "...")]`

示例：
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // your test
    }
}
```

## 性能优化建议

- 避免不必要的内存分配
- 使用批处理减少 I/O 操作
- 考虑使用缓冲区池（buffer pool）
- 性能测试使用 `cargo bench`（如有基准测试）

## 文档更新

当你的更改影响以下内容时，请更新对应文档：
- `README.md`：用户功能、安装、使用
- `CHANGELOG.md`：版本变更记录
- `docs/quickstart.md`：快速开始指南
- `docs/architecture.md`：架构设计
- 代码注释：公共 API、复杂逻辑

## 报告问题

在 GitHub Issues 中报告 bug 或建议功能时，请包含：
- 问题描述
- 复现步骤
- 预期行为 vs 实际行为
- 环境信息（OS、Rust 版本等）
- 日志输出或错误信息

## 行为准则

- 尊重他人
- 保持建设性讨论
- 专注于问题本身，而非个人

## 许可证

贡献的代码将遵循项目的 Apache-2.0 许可证。

感谢你的贡献！

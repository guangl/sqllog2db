# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.5] - 2026-04-13

### Changed

- **依赖升级**：`dm-database-parser-sqllog` 从 `0.9.0` 升级至 `0.9.1`，解析层单线程性能提升约 20.5%
  - 上游优化：`find_indicators_split()` 中的 `memrchr` 循环改为预构建 SIMD `FinderRev`
  - UTF-8 文件跳过每条记录的 `simdutf8` 重复校验（已在 `from_path` 阶段完成）
  - `EXECTIME` 浮点解析从 `str::parse::<f32>()` 改为 `fast-float`
- **`:N` 占位符数字解析优化**：`count_placeholders` 与 `apply_params_into` 中的序号解析从 `from_utf8 + str::parse::<usize>()` 改为直接字节累加，消除 UTF-8 验证和字符串 parse 开销

### Performance

- `filters/trxid_small`：**−7.5%** 耗时（约提升 8.1%）
- `filters/trxid_large`：**−7.6%** 耗时（约提升 8.2%）
- `filters/no_pipeline`：**−2.0%** 耗时
- `filters/pipeline_passthrough`：**−1.7%** 耗时
- `filters/indicator_prescan`：**−2.3%** 耗时

---

## [0.10.2] - 2026-04-12

### Changed

- 流式导出路径全面替换批量导出路径，移除 `export_batch` / `export_batch_with_normalized` 接口及 `Pipeline::run` 死代码
- 性能优化：`memchr3` 快速扫描、`normalize` 快速跳过、`ahash` 哈希表、批量写入调优

---

## [0.10.0] - 2026-04-09

### Added

- **`digest` 子命令**：SQL 指纹聚合分析。将字面量（字符串、数字）替换为 `?`，结构相同的 SQL 折叠为同一指纹，输出执行次数、总/平均/最大执行时间及代表 SQL，快速定位高频或高耗查询
  - `--top N`：只显示前 N 条指纹
  - `--sort count|exec`：按执行次数（默认）或总执行时间排序
  - `--min-count N`：忽略出现次数低于 N 的指纹
  - `--json`：JSON 格式输出，适合管道处理
  - 支持 `--from`/`--to` 时间范围过滤
- **`stats --group-by`**：按维度聚合统计（`user`、`app`、`ip`，可叠加），输出每组的记录数、总/平均/最大执行时间
- **`stats --bucket`**：按时间粒度分桶统计（`hour`、`minute`），含迷你柱状图可视化
- **`stats --json` 扩展**：JSON 输出现包含 `group_sections` 和 `time_buckets` 字段

### Changed

- `stats` 函数签名新增 `group_by` 和 `bucket` 参数

---

## [0.9.0] - 2026-04-05

### Added

- **Glob 输入支持**：`sqllog.path` 现在接受 glob 模式（如 `./logs/2025-*.log`），自动匹配所有 `.log` 文件
- **目录扫描排序**：目录模式和 glob 模式扫描结果均按路径字典序排序，确保处理顺序可预期
- **`stats --top N`**：展示执行时间最长的前 N 条慢查询；使用有界 min-heap 实现，无论日志大小均为常数内存
- **`stats -v` 每文件明细**：verbose 模式下按文件输出记录数、解析错误数及处理速率

### Changed

- **`sqllog.directory` 重命名为 `sqllog.path`**：新字段名更准确地反映其支持文件/目录/glob 三种输入形式；旧字段名通过 `serde(alias)` 保持向后兼容，`--set sqllog.directory=...` 同样继续有效

---

## [0.8.0] - 2026-04-05

### Removed

- **移除 JSONL 导出器**：删除 `src/exporter/jsonl.rs` 及所有相关配置（`[exporter.jsonl]`），导出格式精简为 CSV 和 SQLite
- **移除 ErrorLogger**：不再将解析错误写入独立文件，改为通过 `log::trace!()` 写入应用日志；同步移除 `[error]` 配置节
- **移除全部 Cargo Features**：删除 `csv`、`jsonl`、`sqlite`、`filters`、`replace_parameters`、`full` 六个编译开关，所有功能始终编译进二进制

### Added

- **`validate --set`**：`validate` 子命令新增 `--set KEY=VALUE` 选项，可在校验前覆盖配置字段
- **`stats --from / --to`**：`stats` 子命令支持时间范围过滤，与 `run` 命令保持一致
- **`validate` 命令提取为独立处理函数**：`cli::validate::handle_validate` 负责输出生效配置详情（路径、日志级别、过滤器、导出器）
- **156 个新测试**，行覆盖率从 ~53% 提升至 **74.82%**
- **CI 覆盖率门控**：`cargo-llvm-cov` 检查行覆盖率 ≥ 70%
- **CI 性能基准检查**：release 模式断言 CSV 吞吐量 ≥ 500k 条/秒

### Performance

当前基准（Apple M 系列，50k 条合成数据）：

| 导出器 | 吞吐量 |
|--------|--------|
| CSV | ~2.13M 条/秒 |
| SQLite | ~1.11M 条/秒 |
| Filters 预扫描 | ~2.25M 条/秒 |

---

## [0.7.0] - 2026-04-04

### Added

- **`--no-color` 全局标志**：强制关闭 ANSI 颜色输出；同时兼容 `NO_COLOR` 环境变量
- **`SQLLOG2DB_CONFIG` 环境变量**：所有带 `--config` 参数的子命令（`run`、`validate`、`show-config`、`stats`）均可通过该环境变量指定配置文件路径
- **`run` 结束后错误摘要**：存在解析错误时，在完成摘要行下方额外打印错误条数及错误日志路径（`⚠ N parse errors logged → path`）
- **`stats` 子命令**：无需导出即可统计日志目录下所有文件的记录总数和解析错误数，并显示处理速率与耗时
- **preflight 预检（`run` 前）**：启动导出前自动校验日志目录存在性及输出文件可写性；`--dry-run` 模式下跳过预检

---

## [0.6.0] - 2026-04-04

### Added

- **进度条**：`run` 命令新增实时 spinner 进度条（`indicatif`），显示当前文件、已处理记录数及速率；`--quiet` 模式下自动隐藏
- **`--limit N`**：跨文件限制最多导出 N 条记录，方便快速抽样验证
- **`--dry-run`**：解析所有日志但不写任何文件，仅统计记录数，方便估算数量或调试配置
- **`--set KEY=VALUE`**：命令行覆盖配置文件中的任意字段（点路径，如 `exporter.csv.file=out.csv`），无需修改配置文件即可临时调整参数
- **`show-config` 子命令**：以着色结构体形式展示当前生效配置（含 `--set` 覆盖后的值），方便排查配置问题
- **`--from / --to` 日期范围过滤**：在 `run` 命令中直接指定时间范围，等价于配置文件中的 `features.filters.meta.start_ts/end_ts`（需 `filters` feature）
- **彩色输出**：终端环境下进度条和摘要使用 ANSI 颜色，支持 `NO_COLOR` 环境变量关闭
- **Ctrl+C 优雅退出**：捕获 SIGINT 后在当前 batch 结束时停止，已处理数据正常 finalize，退出码 130（128+SIGINT）
- **有意义的 exit code**：2=配置错误，3=I/O/解析错误，4=导出错误，130=用户中断
- **man page 生成**：新增 `man` 子命令，将 man page 内容输出到 stdout（`sqllog2db man > sqllog2db.1`）

### Changed

- `run` 命令日志不再输出到 stdout，只写文件，防止与进度条交叉污染
- `--quiet` 模式完整抑制所有非错误输出（进度条、摘要、统计行）

### Testing

- 新增 `tests/coverage_boost_tests.rs`，覆盖 SQLite exporter、JSONL 记录导出、show_config、config apply_overrides、color 模块
- 测试覆盖率从 57% 提升至 80%

---

## [0.5.0] - 2026-04-04

### Added
- **`replace_parameters` feature**：将 `PARAMS(SEQNO, TYPE, DATA)` 参数值替换进 SQL 占位符，在导出结果中新增 `normalized_sql` 列
  - 支持 `?` 顺序占位符和 `:N` 序号占位符（Oracle 风格）两种风格，可通过 `placeholders` 数组配置，空数组自动检测
  - 支持 INS / DEL / UPD / SEL 四种执行记录类型
  - 替换前校验 PARAMS 数量与占位符数量一致性，不符则写入警告日志跳过替换
  - buffer 采用消耗语义（`remove`），防止达梦 `stmt` 内存地址复用导致跨 SQL 参数污染

### Fixed
- **SQLite 性能回退**：`do_insert` 重构后 `prepare_cached` 被移入循环内（每条记录一次），恢复为循环外单次 prepare + 复用 `CachedStatement`，50000 条耗时 40ms → 34ms

### Performance
- SQLite 导出较 0.4.3 基线提升约 10%

---

## [0.4.3] - 2026-04-04

### Changed
- 升级 `dm-database-parser-sqllog` 0.7.0 → 0.9.0，解析层改用 mmap + SIMD 加速，性能全面提升

### Performance
- **全导出器提速**：CSV -9~11%，JSONL -26~30%，SQLite -8~10%（基于 Criterion 对比 0.7.0 baseline）
- **pre-scan 并行化**：`filters` feature 下事务扫描阶段改用 `par_iter()` 多核并行，prescan -11%；`rayon` 作为可选依赖随 `filters` feature 一同启用

---

## [0.4.1] - 2026-04-03

### Performance

- **过滤器零开销快速路径**：当 `filters` 未配置或未启用时，主循环完全跳过过滤逻辑，不再产生任何运行时开销。
- **重构 `Pipeline` 架构**：将过滤逻辑从 `process_log_file` 中抽出，封装为独立的 `FilterProcessor` + `Pipeline`，热循环中仅保留 `pipeline.is_empty()` 判断。
- **修复 `enable` 标志未生效**：之前即使 `enable = false`，过滤条件仍会执行；现已确保只有在 `has_filters()` 为真时才将处理器加入管线。

## [0.4.0] - 2026-04-03

### Changed
- **全面重构**：以简洁、可读性、极致性能和易扩展性为目标对代码库进行全面重构。
  - 移除 `rayon` 依赖，消除了 CSV 导出中 `par_iter().flat_map(Vec<u8>)` 按字节并行的隐藏 bug（CSV 导出速度从 ~7.64s 提升至 ~0.56s）
  - 将 `export_batch` 接口签名从 `&[&Sqllog]` 改为 `&[Sqllog]`，消除每批次的 `Vec<&Sqllog>` 额外分配
  - JSONL 导出器改用借用版 `JsonlRecord<'a>` + `serde_json::to_writer` 直写，消除逐条 String 分配（~1.60s）
  - CSV 导出器改用 `itoa::Buffer` + `line_buf: Vec<u8>` 跨条目复用，零额外分配（~0.56s）
  - SQLite 导出器直接传 `&str` 给 rusqlite 参数，消除逐条 `.to_string()` 分配（~1.11s）

### Removed
- **简化错误类型**：将 `ExportError::CsvExportFailed`、`FileCreateFailed`、`FileWriteFailed` 合并为单一 `WriteError { path, reason }`
- **移除空类型**：删除无内容的 `DatabaseError`、`ParseError` 枚举及对应 `From` 实现
- **简化 `ErrorLogger`**：移除 `ErrorMetrics`、`ParseErrorRecord` 中间结构，直接逐行写入
- **简化 `Config`**：移除所有 getter 方法（`.directory()`、`.file()`、`.level()` 等），直接访问公开字段
- **移除 `ProcessContext`**：简化 `Pipeline::run` 接口，移除无用的上下文参数
- **移除 `anyhow` 可选依赖**

### Fixed
- 修复所有 `cargo clippy --all-targets --all-features -- -D warnings` 警告，并加入 pre-commit hook 强制执行

## [0.3.2] - 2026-04-01

### Added
- **模块化特性开关**：将 `filters` 和 `replace_parameters` 模块集成为可选的 Cargo Features。
  - `filters`：支持 SQL 记录级和事务级过滤。
  - `replace_parameters`：支持 SQL 参数占位符替换（依赖可选的 `anyhow` 库）。
  - `full`：新增一键开启所有功能特性的快捷开关。
- **条件编译优化**：针对不同特性组合进行了深度的条件编译优化，显著减小了在禁用特定功能时的二进制体积和运行开销。
- **时间范围过滤**：支持 `start_ts` 和 `end_ts` 过滤字段，允许按 SQL 日志的时间戳范围进行过滤。
- **配置结构优化**：将过滤相关的元数据字段平铺到 `[features.filters]` 下，简化了 `config.toml` 的编写。
- **智能过滤启用**：当检测到任何过滤器配置时，程序将自动应用过滤逻辑，不再强制要求 `enable = true`。

### Fixed
- **编译告警消除**：修复了在禁用 `filters` 特性时出现的“未使用变量”和“冗余 clone”等编译警告。
- **时间戳比较优化**：改进了 `ts` 过滤的字符串比较逻辑，支持毫秒级的精确匹配与范围包含。
- **配置解析健壮性**：为配置结构增加了更多默认值，防止因缺少非必填字段导致的解析失败。
- **代码质量**：修复了多处 Clippy 警告，包括冗余闭包和字段初始化优化。


## [0.3.1] - 2026-04-01

### Added
- **SQL Tag 支持**：同步获取并导出达梦数据库 SQL 日志中的 Tag 标签。
- **Tag 过滤**：在 `FiltersFeature` 中新增按 Tag 标签进行记录级过滤的功能。
- **导出器增强**：CSV、JSONL 和 SQLite 导出器均已支持 Tag 字段。

## [0.3.0] - 2026-04-01

### Removed
- **架构精简**：大幅精简导出器逻辑，移除了以下支持以降低维护成本：
  - Parquet 导出器
  - DuckDB 导出器
  - PostgreSQL 导出器
  - DM 导出器
  - Oracle 导出器
- **特性开关**：移除对应的 `parquet`、`duckdb`、`postgres` 特性开关。
- **文档与示例**：清理了所有已废弃导出器的示例配置和文档。

### Fixed
- **Clippy**：修复 `src/config.rs` 中的 `non-minimal-cfg` 警告。

## [0.2.1] - 2025-12-07

### Fixed
- **代码质量改进**：
  - 修复所有 `cargo clippy` 告警（默认特性和所有特性模式）
  - 移除 `unreadable_literal` 警告（数字分隔符）
  - 修复 `unused_variables`、`dead_code`、`format_push_string` 等编译告警
  - 修复结构体更新语法中的冗余 `..Default::default()`
  - 改进字符串文字比较（使用 `contains()` 替代 `ends_with()`）
  - 使用 `writeln!()` 替代 `format!() + push_str()`

### Changed
- 测试框架增强：
  - 调整所有测试以兼容默认特性和完整特性两种编译模式
  - 添加 `#[allow(clippy::needless_update)]` 来处理特性条件编译下的结构体更新

### Testing
- ✅ 所有 690+ 测试通过
- ✅ `cargo clippy --all-targets -- -D warnings` 通过
- ✅ `cargo clippy --all-targets --all-features -- -D warnings` 通过
- ✅ `cargo fmt --all -- --check` 通过
- ✅ `cargo doc --all-features` 通过

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
- **进度报告**：CLI 运行时展示文件处理进度（X/Y）
- **性能统计**：导出完成后显示总耗时、记录数、吞吐量（条/秒）

### Changed
- **Cargo.toml**：添加 `authors` 字段，增加 `clap_complete` 依赖
- **CLI 描述优化**：更详细的 `about` 和 `long_about` 说明，含完整命令示例
- 默认配置模板与代码默认值对齐：`sqllog.directory = "sqllogs"`、错误日志默认输出到 `export/errors.log`（按行文本）
- README 配置示例与导出器优先级、默认路径保持一致
- 导出器优先级警告信息更详细（包含完整优先级列表）
- **代码质量**：
  - 移除所有 `unsafe` 代码，改用 Rust safe 抽象
  - 移除所有 `#[allow]` 属性，修复底层代码问题
  - 所有导出器实现 `Debug` trait（无 `#[derive]` 辅助）
  - 用 `std::sync::LazyLock`（Rust 1.80+）替换 `once_cell::sync::Lazy`，减少依赖
- **内存优化**：减少不必要的 `clone()` 调用

### Removed
- 移除未使用的 `dashmap` 依赖
- 移除所有 `#[allow]` 属性：`dead_code`、`unused_fields`、`missing_debug_implementations` 等
- 移除 `once_cell` 依赖（采用 Rust 1.80+ 原生 `LazyLock`）

### Fixed
- `oracle.rs` 中用 `ok_or_else` 替换 `unwrap()`，提高错误处理健壮性
- 移除 DmExporter 中未使用的 `overwrite` 和 `charset` 字段
- 移除 `DatabaseError::Charset` 变量（未使用）
- 移除 `default_charset()` 函数（未使用）

### Performance
- 内存优化：总峰值内存从 2.42GB 降至约 179MB（-92.6%），Parquet 峰值从 2.37GB 降至 ~134MB
- 批处理与块处理参数调整（500/1000）保持或提升导出性能
- 二进制体积：2.8MB（Windows，已启用 LTO、strip、panic=abort）
- **零编译警告**：所有 clippy 警告已消除，代码质量达到生产级


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

[0.9.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.9.0
[0.8.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.8.0
[0.7.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.7.0
[0.6.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.6.0
[0.5.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.5.0
[0.4.3]: https://github.com/guangl/sqllog2db/releases/tag/v0.4.3
[0.4.1]: https://github.com/guangl/sqllog2db/releases/tag/v0.4.1
[0.4.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.4.0
[0.3.2]: https://github.com/guangl/sqllog2db/releases/tag/v0.3.2
[0.3.1]: https://github.com/guangl/sqllog2db/releases/tag/v0.3.1
[0.3.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.3.0
[0.2.1]: https://github.com/guangl/sqllog2db/releases/tag/v0.2.1
[0.2.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.2.0
[0.1.2]: https://github.com/guangl/sqllog2db/releases/tag/v0.1.2
[0.1.1]: https://github.com/guangl/sqllog2db/releases/tag/v0.1.1
[0.1.0]: https://github.com/guangl/sqllog2db/releases/tag/v0.1.0

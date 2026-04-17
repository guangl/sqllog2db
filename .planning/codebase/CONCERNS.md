# Codebase Concerns

**Analysis Date:** 2026-04-17

## Tech Debt

**SQLite 使用激进 PRAGMA，牺牲持久性换性能：**
- Issue: `PRAGMA journal_mode = OFF` + `PRAGMA synchronous = OFF` 意味着进程崩溃时数据库可能损坏
- Files: `src/exporter/sqlite.rs:216-224`
- Impact: 意外中断（如 OOM、kill -9）后 SQLite 文件可能无法打开
- Fix approach: 改为 `WAL` + `synchronous = NORMAL`，仍远快于默认值，但具备崩溃安全性

**`conn.as_ref().unwrap()` 在生产路径中多次出现：**
- Issue: SQLite exporter 的 `export()`/`export_one_normalized()`/`export_one_preparsed()` 三个方法都通过 `ok_or_else(|| Self::db_err("not initialized"))` 处理未初始化问题，但 `initialize()` 内部还有两处 `self.conn.as_ref().unwrap()`（行 231、236）紧跟在 `self.conn = Some(conn)` 之后——这两处是安全的，但维护时容易混淆
- Files: `src/exporter/sqlite.rs:231`, `src/exporter/sqlite.rs:236`, `src/exporter/sqlite.rs:243`
- Impact: 若未来重构顺序改变，可能引入 panic
- Fix approach: 提取 `fn conn_ref(&self) -> Result<&Connection>` 统一处理

**`apply_one` 配置覆盖 key 列表是硬编码白名单：**
- Issue: 添加新配置项后必须同步更新 `Config::apply_one` 的 `match` 分支，否则 `--set` 不生效，编译时不报错
- Files: `src/config.rs:111-168`
- Impact: 新增配置字段（如 `features.*`）无法通过 `--set` 覆盖，容易被遗漏
- Fix approach: 文档注释标注"修改时同步更新 apply_one"，或迁移为 serde-based 动态路径方案

**`ResumeState.processed` 使用线性搜索：**
- Issue: `is_processed()` 对 `processed` Vec 做全量遍历，`mark_processed()` 先 `retain` 后 `push`，均为 O(n)
- Files: `src/resume.rs:69-79`, `src/resume.rs:82-99`
- Impact: 文件数量非常大（数千个）时性能下降。当前典型使用场景（<数百个文件）不受影响
- Fix approach: 将 `processed` 改为 `HashMap<String, ProcessedFile>`

## Known Bugs / 边界情况

**并行 CSV 模式下中断后不更新 resume state，但顺序模式逐文件写入：**
- Symptoms: 并行模式被 Ctrl-C 打断后，`--resume` 下次运行会重处理所有文件
- Files: `src/cli/run.rs:688-696`
- Trigger: `--jobs >1` + `--resume` + Ctrl-C
- Workaround: 无；设计决策：并行任务中无法区分"完整处理"与"中途截断"，保守不标记

**SQL 记录级过滤（`record_sql`）只对有 `tag` 的 DML 记录生效，PARAMS 记录始终通过：**
- Symptoms: 过滤规则设置 `record_sql` 时，对应 PARAMS 记录仍进入 params_buffer，不影响正确性，但可能让用户误认为"所有 SQL 已过滤"
- Files: `src/cli/run.rs:172-178`
- Trigger: 同时启用 `record_sql` 过滤和参数替换
- Workaround: 行为有意为之（确保参数替换能正确工作），但缺乏文档

**`concat_csv_parts` 在拼接过程中如磁盘满，临时 part 文件已被删除但输出文件可能不完整：**
- Symptoms: 拼接中途失败后，临时目录被清理，输出 CSV 内容被截断
- Files: `src/cli/run.rs:405`, `src/cli/run.rs:566-568`
- Trigger: 磁盘空间不足时并行 CSV 输出
- Workaround: 无自动回滚；用户需手动检查输出文件完整性

## Security Considerations

**SQLite `table_name` 直接拼接进 SQL 语句，无校验：**
- Risk: 若 `table_name` 来自不可信输入（如命令行），可能导致 SQL 注入
- Files: `src/exporter/sqlite.rs:34-35`, `src/exporter/sqlite.rs:52-100`, `src/exporter/sqlite.rs:232`, `src/exporter/sqlite.rs:237`
- Current mitigation: `table_name` 来自 config 文件，用户自行编辑，非直接网络输入；CLI 无对应命令行参数
- Recommendations: 添加 `table_name` 合法标识符校验（仅允许 `[A-Za-z_][A-Za-z0-9_]*`），或使用 `rusqlite` 引号转义

**`unsafe_code = "warn"` 而非 `"deny"`：**
- Risk: 现有代码未使用 unsafe，但 warn 级别允许在不通知 CI 的情况下引入 unsafe 块
- Files: `Cargo.toml:71`
- Current mitigation: 无
- Recommendations: 改为 `unsafe_code = "deny"`，强制所有 unsafe 通过 `#[allow(unsafe_code)]` 显式标注

## Performance Concerns

**`stats` 命令内存无界增长（group_maps + bucket_map）：**
- Problem: `HashMap<String, GroupAccumulator>` 和 `BTreeMap<String, BucketAccumulator>` 在 `handle_stats` 中随文件数量无限累积，无驱逐或上限
- Files: `src/cli/stats.rs:265-267`
- Cause: 每个唯一 user/app/ip/时间桶都占一个条目；日志量极大时（数百万不重复 IP）会耗尽内存
- Improvement path: 添加 `--max-groups` 上限，超出时退化为 Top-N 近似统计

**参数替换的 `ParamBuffer` 无过期清理：**
- Problem: `params_buffer: HashMap` 内存随未被消耗的 PARAMS 条目累积（日志格式异常时 PARAMS 无对应 DML）
- Files: `src/cli/run.rs:119`（仅在文件边界 `clear()`），`src/features/replace_parameters.rs:390`（`remove` 消耗条目）
- Cause: 若一个文件内有大量无对应 DML 的 PARAMS 记录，它们会驻留到文件处理结束
- Improvement path: 超出阈值（如 10000 条）时清理最老条目；或记录到错误日志

**`ExportStats.flush_operations` / `last_flush_size` 字段被统计但从未被读取或用于决策：**
- Problem: 两个字段增加了内存和维护成本，但没有被任何输出或逻辑消费
- Files: `src/exporter/mod.rs:108-113`
- Improvement path: 删除冗余字段，或接入 `log_stats()` 展示

## Fragile Areas

**`GroupBy::from_str` 在 JSON 输出路径中做了 `unwrap_or(GroupBy::User)` 静默回退：**
- Files: `src/cli/stats.rs:407`
- Why fragile: 若序列化的 `section.field` 字符串与 `from_str` 期望的值不匹配（如未来添加新枚举变体），会静默回退为 User 分组，输出错误数据
- Safe modification: 改为 `unwrap_or_else(|| panic!(...))` 或返回 `Option`，主调方提前校验

**`FieldMask` 硬编码位数 = 15，若新增导出字段需同步修改多处：**
- Files: `src/features/mod.rs:38`（`ALL = 0x7FFF`）, `src/exporter/csv.rs`（逐字段 `is_active(N)` 分支）, `src/exporter/sqlite.rs:74-90`（`COL_TYPES` 数组）
- Why fragile: 新增字段时 3 处都要修改，任何一处漏改均导致导出列错位或缺列，编译期无提示
- Safe modification: 将字段定义集中到一处（如 proc-macro 或 const 数组），其他处从中派生

**时间过滤使用字典序字符串比较，依赖日志时间戳格式固定：**
- Files: `src/cli/run.rs:69-76`, `src/features/filters.rs:123-131`
- Why fragile: 若上游 `dm-database-parser-sqllog` 改变时间戳格式（如添加时区信息），字典序比较会产生错误结果
- Safe modification: 添加时间戳格式校验，或改为解析为 `chrono::DateTime` 后比较

## Scaling Limits

**并行 CSV 处理的临时 part 文件数量 = 日志文件数量：**
- Current capacity: 每个日志文件生成一个临时 CSV，无上限
- Limit: 数千个日志文件时会创建数千个临时文件，可能触及 OS 文件描述符或 inode 限制
- Scaling path: 批量处理 part（N 个文件合并为一个 part），或使用 pipe 直接流式拼接

**`ResumeState.processed` 列表无大小限制，随处理文件数线性增长：**
- Current capacity: 序列化为 TOML 文件，文件越大加载越慢
- Limit: 数万个文件后 TOML 状态文件体积达 MB 级
- Scaling path: 迁移为 SQLite 或二进制格式存储 resume state

## Dependencies at Risk

**`dm-database-parser-sqllog = "0.9.1"` 是内部私有依赖：**
- Risk: 所有解析逻辑（时间戳格式、字段含义、PARAMS 语法）均封装在此 crate，无法在本仓库中修改；破坏性变更会级联影响过滤、参数替换、stats 等所有功能
- Impact: 升级版本需全面回归测试；无法通过 CI 感知上游破坏性 API 变更（semver 保护有限）
- Migration plan: 为关键解析行为（MetaParts 字段、PARAMS 格式）添加集成测试，使破坏性变更可见

**`self_update = "0.44.0"` 引入了 `reqwest` 和 TLS 依赖：**
- Risk: 增加了攻击面（HTTP 客户端 + TLS 库），每次网络更新都执行来自 GitHub releases 的二进制
- Impact: 依赖链较重，但功能非核心路径；更新失败不影响主功能
- Migration plan: 考虑将 `self_update` 移到 optional feature，减小默认二进制体积和依赖

## Test Coverage Gaps

**`cli/run.rs` 中的 `process_log_file` 和 `handle_run` 没有直接单元测试：**
- What's not tested: 主流程中的 limit 截断、resume 跳过、中断信号处理路径
- Files: `src/cli/run.rs:102-264`, `src/cli/run.rs:580-804`
- Risk: 重构管线逻辑时可能引入静默的数量偏差（多计/少计记录）
- Priority: High

**并行 CSV 路径（`process_csv_parallel`）无集成测试：**
- What's not tested: 多文件并行写入、临时 part 合并顺序、错误中途清理
- Files: `src/cli/run.rs:418-578`
- Risk: 并发条件下的文件排序错误或 header 重复无法被单测发现
- Priority: High

**`concat_csv_parts` 的错误路径（磁盘满、文件已删除）无测试覆盖：**
- What's not tested: 写入失败后的临时文件清理行为
- Files: `src/cli/run.rs:362-411`
- Risk: 错误路径可能泄露临时目录
- Priority: Medium

**`stats` 命令的过滤器联合使用场景未测试：**
- What's not tested: 同时启用 `group_by` + `bucket` + `--top` 时的数据一致性
- Files: `src/cli/stats.rs:200-414`
- Risk: 多个聚合维度共享 mutable state 时可能出现计数不一致
- Priority: Medium

---

*Concerns audit: 2026-04-17*

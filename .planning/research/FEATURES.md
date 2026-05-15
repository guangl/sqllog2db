# Feature Research

**Domain:** CLI log processing tool — exclusion filters, hot-path optimization, CLI startup speed
**Researched:** 2026-05-10
**Confidence:** HIGH (analysis from existing codebase) / MEDIUM (external UX patterns)

---

## Feature Landscape

### Table Stakes (Users Expect These)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| FILTER-03: 元数据字段排除模式 | 现有 include 支持正则，exclude 是对称补集；`record_sql`/`SqlFilters` 已有 `exclude_patterns`，用户自然期待元数据字段也有同等能力 | LOW | 只需在 `MetaFilters` / `CompiledMetaFilters` 中镜像 `exclude_*` 列表，并在 `should_keep` 中增加排除短路判断 |
| FILTER-03: include 与 exclude 可同时配置 | 工业级日志工具（CloudWatch、Loki、grep -v）均支持"先选入再排除"语义 | LOW | include 通过后再检查 exclude，顺序清晰；和已有 `CompiledSqlFilters.matches()` 逻辑一致 |
| FILTER-03: 空 exclude 不影响性能 | 无配置场景必须保持零开销 | LOW | `None` 短路已是当前 `match_any_regex` 惯例，直接复用 |
| PERF-10: 热路径剩余瓶颈标定 | v1.1 已做 profiling 但改进留有空间；用户期待每版继续推进 | MEDIUM | 需基于 flamegraph + criterion 实测，不凭猜测 |
| PERF-11: `sqllog2db validate` / `run` 冷启动可感知延迟应最小 | 交互式使用时，等待 >100ms 会被感知为"卡顿" | LOW-MEDIUM | Rust 冷启动本身极快（<5ms），主要开销在：TOML 解析、regex 编译、文件 I/O（log 目录扫描）、update check 网络请求 |
| DEBT-01: sqlite.rs 错误不静默吞掉 | 错误静默 = 数据丢失无感知，对任何导出工具不可接受 | LOW | 现有 `error log` 基础设施已就绪，只需把 `let _ =` 和 `unwrap_or(())` 替换为记录到 error log |
| DEBT-02: table_name SQL 注入白名单校验 | 用户可在 config 填写任意 `table_name`，当前直接字符串拼接 SQL，存在注入面 | LOW | 简单白名单正则 `^[A-Za-z_][A-Za-z0-9_]*$` 即可；或在 `validate()` 阶段拦截 |

### Differentiators (Competitive Advantage)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| FILTER-03: 排除模式对所有 7 个元数据字段均生效 | 竞品（grep -v）只能全行排除；本工具可精确按 username/ip/appname 等字段排除，粒度更细 | LOW | 字段集合固定（trxid、ip、sess、thrd、user、stmt、app、tag），逐字段对称扩展 |
| FILTER-03: include + exclude 在同一字段可并存 | 例：username 匹配 `^admin` 但排除 `admin_readonly`，一次配置完成，无需两次运行 | LOW | 语义：include（字段内 OR）全部通过后，再检查 exclude（字段内 OR 任一命中即丢弃） |
| PERF-10: 基于实测 flamegraph 的精确优化 | 不猜测热点，从数据驱动改进，每项优化有 criterion 数据支撑 | MEDIUM | 依赖 `cargo flamegraph` + 真实 1.1GB 日志文件 |
| PERF-11: `validate` 命令亚秒响应 | CLI 工具"立即响应"是 UX 基线；避免 update check 网络请求阻塞配置校验路径 | LOW | update check 已在 `!quiet` 分支，可进一步改为异步/延迟 check |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| OR 语义的排除组合（exclude A OR exclude B 在跨字段层面） | 用户有时想"排除 userA 或排除 ip X"——跨字段的 OR 排除 | 大幅增加配置复杂度，与现有 AND 语义混淆，测试矩阵爆炸 | 当前字段内 OR 已足够：同字段多 pattern 是 OR，跨字段是 AND；这一约束明确标注在 PROJECT.md Out of Scope |
| 运行时动态排除规则（热重载） | 有时需要临时屏蔽某个用户 | 状态管理复杂，违反"启动时加载一次"的简单模型 | 修改 config + 重新运行；CONFIG.md 已明确 Out of Scope |
| exclude 对 trxid 字段支持正则 | trxid 是数字字符串，regex 毫无意义 | 引入 `TrxidSet` exclude 的数据结构变化，收益几乎为零 | trxid exclude 可通过字面字符串列表实现（如同 include），保持 `HashSet` 精确匹配 |
| PERF-11: 把 update check 改为后台线程阻塞 | 想并行执行 | 引入线程同步复杂性；update check 对 `run` 命令已被跳过（quiet 路径），收益边际 | 直接在 `!quiet && !run` 路径内做同步 check 已经够快（网络超时即返回） |

---

## Feature Dependencies

```
FILTER-03 元数据字段排除
    └──镜像于──> CompiledMetaFilters（现有 include 结构）
                    └──无需──> 新的编译基础设施（复用 compile_patterns + match_any_regex）
    └──影响──> validate.rs 输出（需展示 exclude 字段数量）
    └──影响──> init.rs 生成的配置模板（需增加注释示例）

PERF-10 热路径优化
    └──依赖──> v1.1 flamegraph/criterion 基础设施（已就绪）
    └──无依赖──> FILTER-03（可并行开发）

PERF-11 CLI 启动提速
    └──独立于──> FILTER-03 / PERF-10
    └──影响──> main.rs run() 的初始化顺序

DEBT-01 sqlite 错误修复
    └──独立于──> 所有 FILTER/PERF 功能

DEBT-02 table_name 校验
    └──属于──> Config::validate() 扩展，独立
```

### Dependency Notes

- **FILTER-03 复用现有基础设施：** `compile_patterns`、`match_any_regex`、`validate_pattern_list` 无需修改，只需在 `MetaFilters`/`CompiledMetaFilters` 中增加 `exclude_*` 字段并在 `should_keep` 末尾追加排除检查。
- **FILTER-03 与 PERF-10 无冲突：** 两者可在不同 phase 独立推进，排除过滤在热路径中是追加的短路判断，不改变现有过滤器的结构。
- **DEBT-01/02 是独立的小任务：** 无外部依赖，任何 phase 均可单独合并。

---

## MVP Definition

### v1.2 必须交付（对应 Active 需求）

- [x] **FILTER-03** — 在 `MetaFilters` 中增加与 include 对称的 exclude 字段（`exclude_usernames`、`exclude_client_ips`、`exclude_sess_ids`、`exclude_thrd_ids`、`exclude_statements`、`exclude_appnames`、`exclude_tags`），编译进 `CompiledMetaFilters`，在 `should_keep` 中作为最后一道关卡（include 全通过 → exclude 任一命中即丢弃）
- [x] **DEBT-01** — sqlite.rs 静默错误改为写 error log
- [x] **DEBT-02** — table_name 白名单校验在 `SqliteExporter::validate()` 中实现

### 条件交付（基于 profiling 结果）

- [ ] **PERF-10** — 必须先运行 flamegraph，若剩余热点 >5% 可消除则实施；否则标注"已达当前瓶颈"跳过
- [ ] **PERF-11** — 用 `hyperfine` 测量 `validate` 冷启动时间；若 >50ms 则调查具体开销点再优化

### 推迟

- 无新增推迟项（v1.2 范围已明确收敛）

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| FILTER-03 元数据排除模式 | HIGH | LOW | P1 |
| DEBT-01 sqlite 静默错误修复 | HIGH（数据正确性） | LOW | P1 |
| DEBT-02 table_name SQL 注入 | HIGH（安全） | LOW | P1 |
| PERF-10 热路径优化 | MEDIUM | MEDIUM（需 profiling） | P2 |
| PERF-11 CLI 启动提速 | LOW-MEDIUM | LOW-MEDIUM | P2 |
| DEBT-03 Nyquist 补签 | LOW（流程合规） | LOW | P2 |

---

## 实现细节说明

### FILTER-03：排除语义的 UX 预期

基于对 grep/ripgrep（`-v`）、AWS CloudWatch（`NOT EXISTS`）、Google Cloud Logging（`!~`）、Grafana Loki（`!~`）的分析，业界一致语义为：

- **exclude 在 include 之后判断**：只有通过 include 的记录才进入 exclude 检查（避免 exclude 空配置把所有记录都吞掉的 Loki bug，见 issue #3523）
- **字段内 OR**：同字段多个 exclude pattern，任意命中即丢弃（与 include 字段内 OR 对称）
- **跨字段 AND**：若 `exclude_usernames` 和 `exclude_client_ips` 均配置，则两者任意一个命中都丢弃该记录（等价于分别排除）
- **空 exclude = 不排除**：`None` 或空列表直接跳过，保持当前 `match_any_regex(None, ...)` 的零开销行为

**配置示例（TOML）：**
```toml
[features.filters]
enable = true
usernames = ["^admin"]          # include: 只保留 admin* 用户
exclude_usernames = ["admin_ro$"] # exclude: 但排除只读账号
exclude_client_ips = ["^10\\.0\\.0\\."]  # 同时排除特定 IP 段
```

**注意：trxid 的排除**应使用字面字符串列表（保持与 include 的 `HashSet` 精确匹配一致），不支持正则——和 include 的语义保持对称。

### PERF-10：热路径优化方法论

当前基准：~5.2M records/sec（CSV synthetic），~1.55M records/sec（真实 1.1GB 文件）。

优化前必须先 profile：
1. `cargo flamegraph --profile flamegraph -- run -c config.toml` 生成火焰图
2. `cargo bench --bench bench_filters` 运行过滤器微基准
3. 识别 >5% 占比的热点后再动手

潜在剩余优化点（基于代码阅读，需实测验证）：
- `parse_meta()` 返回值中字段是否有不必要的堆分配（`CompactString` 已优化，但需确认）
- `regex::Regex::is_match` vs `find` 的选择（`is_match` 已是最轻量调用）
- `pipeline.run_with_meta` 中各处理器顺序：失败率最高的过滤器排最前（短路收益）

**不应该做的**：在没有 flamegraph 数据的情况下猜测瓶颈并重构。

### PERF-11：CLI 启动开销分析

`main.rs::run()` 的冷启动路径依次执行：
1. `std::env::args().collect()` — 几乎零开销
2. `lang::detect(&raw_args)` — 字符串扫描，微秒级
3. `cli::opts::Cli::command()` + `clap` 解析 — 可能 ~1-5ms（clap 4.x 已优化）
4. `cli::update::check_for_updates_at_startup()` — 网络请求，若超时可能阻塞数百毫秒
5. `load_config()` → `std::fs::read_to_string` + `toml::from_str` — 文件 I/O + TOML 解析，通常 <1ms
6. `Config::validate()` → `validate_regexes()` — regex 编译开销在 pattern 数量多时显著

**最高价值优化**：update check 网络请求。当前代码在 `run` 命令的 quiet 模式下已跳过，但 `validate` 命令默认会触发。可考虑：
- 增加 TTL 缓存（上次 check 时间写入临时文件，24h 内不重复请求）
- 或 `validate` 命令默认跳过 update check（不影响功能）

**测量方式**：`hyperfine --warmup 3 'sqllog2db validate -c config.toml'`，目标 <30ms（不含 update check）。

---

## Sources

- 代码库阅读：`src/features/filters.rs`（CompiledMetaFilters、SqlFilters、match_any_regex）
- 代码库阅读：`src/cli/run.rs`（FilterProcessor、hot loop、build_pipeline）
- 代码库阅读：`src/main.rs`（启动序列、update check 位置）
- [Grafana Loki issue #3523](https://github.com/grafana/loki/issues/3523) — 空 exclude pattern 吞掉所有记录的 UX 陷阱（MEDIUM confidence）
- [AWS CloudWatch Filter Pattern Syntax](https://docs.aws.amazon.com/AmazonCloudWatch/latest/logs/FilterAndPatternSyntax.html) — NOT EXISTS / exclude 语义（MEDIUM confidence）
- [rust-lang/regex discussions #960](https://github.com/rust-lang/regex/discussions/960) — regex 热路径是否瓶颈的社区分析（MEDIUM confidence）
- [hyperfine](https://github.com/sharkdp/hyperfine) — CLI 启动时间测量工具（HIGH confidence）
- [The Rust Performance Book - Profiling](https://nnethercote.github.io/perf-book/profiling.html) — flamegraph 方法论（HIGH confidence）

---

*Feature research for: sqllog2db v1.2 — FILTER-03 排除模式、PERF-10/11 热路径与启动优化*
*Researched: 2026-05-10*

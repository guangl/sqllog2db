# Technology Stack

**Project:** sqllog2db v1.2 — FILTER-03 / PERF-10 / PERF-11 / DEBT 增量
**Researched:** 2026-05-10
**Scope:** 仅记录新特性所需的栈变更；已验证依赖（regex、rayon、rusqlite、criterion、mimalloc 等）不重复评估。

---

## 结论：零新 crate 依赖

v1.2 所有特性均可在现有依赖范围内实现。下文逐一说明原因。

---

## FILTER-03：排除模式

### 问题背景

代码审查发现：

- `CompiledSqlFilters`（record_sql 层）已有 `exclude_patterns: Option<Vec<Regex>>`，逻辑完整。
- `CompiledMetaFilters`（元数据层：usernames / client_ips / sess_ids / thrd_ids / statements / appnames / tags）完全缺少排除维度，结构体和 `should_keep()` 均无对应字段。
- `MetaFilters`（配置层）同样无排除字段。

因此 FILTER-03 的实现重心在 MetaFilters / CompiledMetaFilters 层，而非 SQL 层（后者已支持排除）。

### aho-corasick 评估 — 不引入

**版本：** 1.1.4（MEDIUM 置信度，cargo search 验证）

**调研结论：不引入。**

理由：

1. `MetaFilters` 字段（usernames、client_ips 等）的排除逻辑是逐记录对单个短字符串执行正则匹配，pattern 列表在用户配置中通常 <10 条。`regex::Regex::is_match` 在此规模下已足够快（SIMD 自动激活）。
2. `SqlFilters`（事务级预扫描层）用 `str::contains` 做字面量匹配。若 exclude_patterns 超过 ~20 条，aho-corasick 单趟扫描才有明显优势，但实际用户不会配置这么多。
3. 引入 aho-corasick 会增加两套类型（`AhoCorasick` vs `Vec<Regex>`），并需维护"是否字面量匹配"的分支逻辑，增加代码复杂度。
4. `memchr 2.8.0` 已在依赖树中，`regex` 内部已通过 aho-corasick 的 literal 优化路径（via `memchr`）实现 SIMD 加速。

**结论：** MetaFilters 排除语义用与包含相同的 `Vec<Regex>` 模型；`SqlFilters` 字面量排除保持 `str::contains`。无需新增 crate。

### regex-lite 评估 — 不引入

**版本：** 0.1.9（MEDIUM 置信度，cargo search 验证）

**调研结论：不引入。**

理由：

1. regex-lite 的设计目标是"减小二进制体积和编译时间"，以牺牲 Unicode 支持和部分优化为代价。
2. 项目热路径已使用 `regex 1.12.3`，CompiledMetaFilters/CompiledSqlFilters 均在启动时预编译，热循环只调用 `is_match()`，不存在重复编译开销。
3. 用户的 pattern 是任意正则（含 `^`、`\b` 等），regex-lite 不保证完整语义兼容，会引入隐藏语义差异。
4. 现有 `regex` crate 已支持 unicode-case-off 特性来缩减编译物；项目未配置 `features = []`，但已有 LTO fat + opt-level=3，增量收益有限。

**结论：** 保留 `regex 1.12.3`。

### 实现方案（纯代码变更）

```toml
# Cargo.toml — 无变化
```

需要修改的文件：

| 文件 | 变更 |
|------|------|
| `src/features/filters.rs` | `MetaFilters` 增加 `exclude_*` 字段；`CompiledMetaFilters` 增加对应 `Vec<Regex>`；`should_keep()` 增加排除短路逻辑（先包含通过，再检查排除） |
| `src/config.rs` | 无变化（`MetaFilters` 字段通过 serde flatten 自动反序列化） |
| `src/cli/run.rs` | `FilterProcessor` 增加 `compiled_meta_exclude` 字段，`process_with_meta` 中先包含后排除 |

---

## PERF-10：热路径优化

### 现状

v1.1 已完成：flamegraph + criterion 定位，16MB BufWriter、itoa、mimalloc、pipeline.is_empty() 快路径、CompactString 内联 trxid、AHashSet O(1) 查询、process_with_meta MetaParts 复用。

### 新增 crate — 不需要

可探索的纯代码优化（无新依赖）：

1. **`memchr` 直接调用**：已在依赖树。`MetaFilters.match_substring` 目前用 `str::contains`（内部已走 memchr），可尝试对每个 exclude 字段收益验证但不是必须引入新 API。
2. **短路顺序调整**：在 `CompiledMetaFilters::should_keep` 中将最高选择率字段（如 username）排在最前，低基数字段（如 tag）在后。通过 criterion 微基准验证顺序收益，无需新依赖。
3. **branch elision**：确认 `has_meta_filters` 预计算模式对 exclude 字段同样适用——新增 `has_meta_exclude_filters: bool` 预计算字段在 `FilterProcessor` 中，避免热路径 `Option::is_some` 判断。

**结论：** PERF-10 不引入新 crate，利用现有 criterion + flamegraph 基础设施。

---

## PERF-11：CLI 启动 / 配置加载提速

### 测量工具

| 工具 | 用途 | 状态 |
|------|------|------|
| `hyperfine` | 端到端冷启动测量，含统计分析和 warm/cold cache 区分 | 已安装（`/opt/homebrew/bin/hyperfine`），版本 1.20.0 |
| `criterion` | 函数级启动路径微基准 | 已在 dev-dependencies |

**hyperfine 用法（冷启动）：**

```bash
# 清 disk cache（macOS）
hyperfine --prepare 'sudo purge' 'target/release/sqllog2db validate -c config.toml' --warmup 0
# 对比新旧版本
hyperfine 'target/release/sqllog2db-old validate' 'target/release/sqllog2db validate'
```

不需要新增任何 crate 用于启动测量。

### 优化方向（不引入新依赖）

1. **regex 预编译批量合并**：`CompiledMetaFilters::from_meta` 为每个字段分别调用 `Regex::new`，可考虑合并为单个 `RegexSet`（已在 `regex` 标准库中）避免多次自动机构建。需要先用 criterion 验证收益——`RegexSet` 查询不返回 pattern 索引，仅返回"是否命中"，适合 include/exclude 语义。
2. **TOML 解析**：`toml 1.1.2` 已是最新，`serde` 零运行时 overhead，此路径瓶颈可能性低——先测量再决策。
3. **lazy_static / once_cell**：项目中没有全局静态 regex，所有 regex 在 `Config::validate` 后、`Pipeline` 构建时一次性编译，已是最优时机，不需要 lazy_static。

**结论：** PERF-11 不引入新 crate。优化在代码层完成，用 hyperfine（已安装）测量。

---

## DEBT-01/02/03：技术债修复

| 债务 | 实现方案 | 新 crate |
|------|---------|---------|
| DEBT-01：sqlite.rs 静默错误 | 将 `if let Err` 静默改为记录到 error log（已有 log crate） | 无 |
| DEBT-02：table_name SQL 注入 | 白名单校验（仅允许 `[a-zA-Z0-9_]`，regex 已在依赖中），或 `rusqlite` 参数化 DDL（DDL 无法参数化，白名单是正确方案） | 无 |
| DEBT-03：VALIDATION.md 补签 | 文档操作，无代码变更 | 无 |

DEBT-02 补充说明：SQLite DDL（CREATE TABLE）不支持参数绑定，table_name 必须拼接进 SQL 字符串。正确方案是正则白名单校验，复用现有 `regex` 或直接用 `str::chars().all(|c| c.is_alphanumeric() || c == '_')`（更快，零依赖）。

---

## 现有栈确认（无变化）

| 技术 | 版本 | 用途 | 状态 |
|------|------|------|------|
| `regex` | 1.12.3 | 元数据/SQL 正则匹配 | 继续使用 |
| `memchr` | 2.8.0 | 子串搜索内核（regex 依赖链上游） | 继续使用 |
| `ahash` | 0.8.12 | trxid HashSet O(1) 查询 | 继续使用 |
| `compact_str` | 0.9.0 | trxid 内联字符串，消除堆分配 | 继续使用 |
| `criterion` | 0.7.0 | 微基准 | 继续使用 |
| `hyperfine` | 1.20.0 | 端到端冷启动测量 | 已安装，CLI 工具 |
| `toml` | 1.1.2 | 配置解析 | 继续使用 |
| `mimalloc` | 0.1.48 | 全局分配器（热路径加速） | 继续使用 |

---

## 不引入 crate 汇总

| Crate | 理由 |
|-------|------|
| `aho-corasick` | 用户 pattern 数量 <20，regex SIMD 已足够；引入增加类型复杂度 |
| `regex-lite` | 语义不完全兼容；热路径已是预编译一次性代价；不改善运行时性能 |
| `lazy_static` / `once_cell` | 无全局静态 regex 需求；pipeline 构建时机已是最优 |
| 任何 profiling crate | hyperfine 已安装；criterion + flamegraph 已有；无需额外 profiling 依赖 |

---

## 信心评估

| 方向 | 置信度 | 依据 |
|------|--------|------|
| FILTER-03 零新 crate | HIGH | 代码审查：CompiledSqlFilters 已有排除逻辑，MetaFilters 仅缺字段扩展 |
| aho-corasick 不引入 | HIGH | 性能分析：规模不符合 AC 优势区间；regex 内部已用 memchr SIMD |
| regex-lite 不引入 | HIGH | 官方文档：以牺牲 Unicode 为代价；语义差异风险不可接受 |
| hyperfine 可用 | HIGH | 本机验证：`which hyperfine` 确认已安装 1.20.0 |
| PERF-11 零新 crate | MEDIUM | 主要瓶颈尚未测量；现有工具已够，若测量后发现 TOML 解析占主导则需复评 |
| DEBT-02 白名单方案 | HIGH | SQLite DDL 参数化局限已知（官方文档），白名单是标准做法 |

---

## 来源

- [aho-corasick docs.rs 1.1.4](https://docs.rs/aho-corasick/latest/aho_corasick/)
- [regex-lite crates.io 0.1.9](https://crates.io/crates/regex-lite)
- [aho-corasick GitHub — performance discussion](https://github.com/BurntSushi/aho-corasick/discussions/136)
- [regex crate — is aho-corasick applied to literals? #891](https://github.com/rust-lang/regex/issues/891)
- [hyperfine GitHub sharkdp/hyperfine](https://github.com/sharkdp/hyperfine)
- [Rust Performance Book — Benchmarking](https://nnethercote.github.io/perf-book/benchmarking.html)

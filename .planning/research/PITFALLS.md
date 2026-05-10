# Pitfalls Research

**Domain:** Rust CLI — streaming log processor with filter pipeline
**Researched:** 2026-05-10
**Confidence:** HIGH (derived from direct codebase inspection + Rust ecosystem patterns)

---

## Critical Pitfalls

### Pitfall 1: 破坏 `pipeline.is_empty()` 零开销快路径

**What goes wrong:**
FILTER-03 排除过滤器加入管线后，即便用户没有配置任何规则，pipeline 也非空（因为排除处理器被无条件添加），导致所有记录在热循环中都要经过一次过滤器实例的方法调用。原本 ~5.2M records/sec 的 CSV 基准可能降至更低，因为每条记录都要构造 `RecordMeta`（`parse_meta()` 开销）。

**Why it happens:**
开发者在 `build_pipeline()` 中按特性分支添加处理器，忘记检查"规则列表是否实际非空"。写法如下就会踩到：
```rust
// 错误：无论是否配置了排除规则都添加处理器
pipeline.add(Box::new(ExcludeFilterProcessor::new(cfg)));
```

**How to avoid:**
与现有 `FilterProcessor` 保持同样的卫语句模式。在 `build_pipeline()` 中，只有当排除规则列表非空时才向管线添加处理器：
```rust
if let Some(f) = &cfg.features.filters {
    if f.has_exclude_filters() {  // 新增 has_exclude_filters() 方法
        pipeline.add(Box::new(ExcludeFilterProcessor::new(f)));
    }
}
```
`has_exclude_filters()` 必须检查规则列表实际包含元素（`Some(v) if !v.is_empty()`），而不仅仅是字段 `is_some()`。

**Warning signs:**
- 在没有配置任何过滤规则的情况下，基准测试性能比预期下降超过 5%
- `pipeline.is_empty()` 在无过滤配置时返回 `false`（可在单元测试中断言）

**Phase to address:** FILTER-03 实现阶段（第一个）

---

### Pitfall 2: 排除过滤器与包含过滤器语义混淆（AND vs OR vs NOT）

**What goes wrong:**
包含过滤器是"字段内 OR，字段间 AND"（多个字段都必须命中）。排除过滤器的直觉是"命中任意一条规则即丢弃"，但如果照搬包含过滤器的 AND 语义，会变成"所有排除规则都命中才丢弃"，行为完全相反。

现有代码中已有一处 OR/AND 语义混淆的历史教训：`FiltersFeature::should_keep()` 和 `CompiledMetaFilters::should_keep()` 语义相反，前者已被标记 `#[deprecated]`。FILTER-03 若不小心，会产生第二处语义错位。

**Why it happens:**
排除过滤器往往被当作"包含过滤器取反"来实现（`!include_logic`），但包含逻辑是"至少一个字段命中"（OR over fields），取反变成了"所有字段都未命中才丢弃"，这不是预期的"只要某条规则命中就丢弃"。

**How to avoid:**
排除过滤器的正确语义：`any(rules.iter(), |rule| rule.matches(record))` → 丢弃。要用独立实现，不要对包含过滤器取反。在 docstring 中用测试矩阵明确记录：

| 记录 | 包含规则 | 排除规则 | 结果 |
|------|---------|---------|------|
| 匹配包含 | 有 | 无 | 保留 |
| 匹配排除 | 无 | 有 | 丢弃 |
| 两者都匹配 | 有 | 有 | 丢弃（排除优先） |
| 两者都不匹配 | 有 | 有 | 丢弃（未满足包含） |

**Warning signs:**
- 测试用例：配置了排除规则但记录被错误保留
- `CompiledSqlFilters::matches()` 中 `exclude_patterns` 已有正确实现可参考（先过 include，再过 exclude，exclude 命中即返回 false）

**Phase to address:** FILTER-03 实现阶段，设计阶段需先写明语义文档

---

### Pitfall 3: 排除过滤器事务级语义模糊（record-level vs transaction-level）

**What goes wrong:**
现有过滤器分两层：
- 记录级（`MetaFilters` + `CompiledMetaFilters`）：每条记录独立判断
- 事务级（`indicators` + `sql`）：预扫描整个事务，命中则整笔保留

FILTER-03 排除过滤器若作用于元数据字段（user/ip/stmt），是记录级丢弃无歧义。但若排除规则作用于 `sql` 内容或事务指标，存在两种可能语义：
1. 记录级：该条 SQL 记录被丢弃，同事务其他记录正常保留
2. 事务级：整笔事务丢弃，预扫描阶段就排除

如果语义选择错误，用户会发现"按 SQL 内容排除"只排除了部分行，而同一事务的其他行仍然出现。

**Why it happens:**
两遍设计（pre-scan + main pass）的复杂性容易被忽视。开发者默认把排除逻辑放在主扫描的记录级，实际上 SQL 内容过滤在预扫描阶段才有完整的事务视角。

**How to avoid:**
v1.2 的 FILTER-03 需求说的是"匹配则丢弃"，配合现有 `record_sql.exclude_patterns`（已在 `CompiledSqlFilters::matches()` 实现）可直接复用记录级语义。元数据排除（user/ip 等）也应该是记录级。明确在 TOML 文档中声明"排除过滤器是记录级，不影响整笔事务的其他记录"，避免用户混淆。

**Warning signs:**
- 用户反馈"按用户名排除但同一事务其他记录还在"——这其实是预期行为，需要文档明确
- 测试用例要覆盖同一 trxid 下多条记录，其中只有部分被排除的场景

**Phase to address:** FILTER-03 实现阶段，需要在配置文档和 error message 中明确说明

---

### Pitfall 4: `regex::Regex` 在热路径重复编译

**What goes wrong:**
如果 FILTER-03 的排除规则没有在启动时预编译，而是在每条记录的过滤时通过 `Regex::new()` 动态构造，将在 hot loop 中引入 O(n×m) 的正则编译开销，n 为记录数，m 为规则数。`Regex::new()` 内部做 NFA 构造，开销是匹配的数百倍。

**Why it happens:**
开发者在快速原型实现时将编译和匹配写在一起，或者误用了 `lazy_static`/`once_cell` 但作用域错误导致每次调用重建。

**How to avoid:**
跟随现有 `CompiledMetaFilters::from_meta()` 模式：在 `FilterProcessor::new()` 或等价的构造函数中调用 `compile_patterns()`，结果存入 `Vec<Regex>` 字段。`compile_patterns()` 已在 `filters.rs` 中实现并返回 `Option<Vec<Regex>>`，None 表示未配置直接通过。

**Warning signs:**
- flamegraph 显示 `regex::hir` 或 `regex::nfa` 出现在热路径上
- `cargo criterion` 基准：排除过滤器比包含过滤器慢超过 10×

**Phase to address:** FILTER-03 实现阶段；PERF-10 验证阶段

---

### Pitfall 5: SQLite `table_name` 注入修复引入 API 破坏性变更

**What goes wrong:**
DEBT-02 要求修复 `table_name` 的 SQL 注入。如果使用白名单校验（推荐），需要在 `Config::validate()` 或 `SqliteExporter::new()` 中拒绝含特殊字符的表名。但现有测试和用例中是否有使用非标准字符的表名（如空格、unicode）尚未确认。若校验过严（如只允许 `[a-zA-Z0-9_]`），将拒绝用户当前可正常工作的配置，造成隐性破坏性变更。

SQLite 本身支持用双引号引用任意字符的表名（`"my table"`），parameterized query 不适用于标识符（只适用于值）。

**Why it happens:**
SQL 注入防御的标准方案（参数化 query）不适用于 DDL 中的标识符，开发者可能误用或引入不完整的白名单。

**How to avoid:**
推荐方案：白名单校验 + 双引号转义组合：
1. 在 `SqliteExporter::validate()`（即 config 验证）时，检查 `table_name` 只含 `[a-zA-Z0-9_]`，否则返回 `ConfigError::InvalidValue`（有清晰的错误消息）
2. 在 SQL 字符串拼接时仍用双引号包裹表名作为兜底：`"CREATE TABLE IF NOT EXISTS \"{table_name}\" (...)"` — 但注意双引号内的双引号需转义为 `""`
3. **不要**仅用双引号包裹而不做白名单校验，因为 `"DROP TABLE foo; --"` 等注入仍可构造

现有代码只校验了 `table_name` 非空（见 config.rs:397），需要补充字符集校验。

**Warning signs:**
- 校验逻辑只在运行时生效而非启动时，导致大批量导出中途失败
- 现有集成测试 `test_sqlite_basic_export` 使用 `"sqllog_records"`（全字母下划线），可正常通过；新增一个含特殊字符的测试用例来验证拒绝逻辑

**Phase to address:** DEBT-02 修复阶段（最早处理，因为影响所有 SQLite 用户）

---

### Pitfall 6: SQLite `DELETE FROM` 静默错误吞掉真实故障

**What goes wrong:**
`sqlite.rs:265` 中：
```rust
let _ = conn.execute(&format!("DELETE FROM {}", self.table_name), []);
```
这条语句（非追加/非覆盖模式下的清空表）的错误被显式丢弃。如果表不存在（首次运行），`DELETE FROM` 会返回错误，静默丢弃是合理的。但如果表存在而删除失败（如 EXCLUSIVE lock 冲突），用户将在后续写入时遇到不清晰的错误，或者以为清空成功实际没有。

**Why it happens:**
开发者判断"表可能不存在"是正常情况，所以用 `let _ =` 丢弃错误。但没有区分"表不存在（无害）"和"删除失败（有害）"这两种 error case。

**How to avoid:**
按 SQLite error code 区分：
```rust
match conn.execute(&format!("DELETE FROM \"{}\"", self.table_name), []) {
    Ok(_) => {}
    Err(e) if e.sqlite_error_code() == Some(rusqlite::ErrorCode::Unknown) => {
        // 表不存在，正常（CREATE TABLE IF NOT EXISTS 会处理）
    }
    Err(e) => {
        // 真实错误：记录到 error log，或者返回 Err
        log::warn!("DELETE FROM {} failed: {e}", self.table_name);
    }
}
```
DEBT-01 修复时，至少应将非预期错误 `warn!` 到 error log，而不是完全静默。

**Warning signs:**
- 用户报告"非覆盖模式下旧数据仍在"，实际是 DELETE 失败了
- `finalize()` 中 `COMMIT` 成功但表中记录数与预期不符

**Phase to address:** DEBT-01 修复阶段

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `let _ = conn.execute(...)` 吞错误 | 代码简洁，忽略表不存在的正常情况 | 真实错误静默丢失，调试困难 | 仅当确认错误是无害的特定 error code，且有注释说明 |
| `format!("... {table_name} ...")` 直接拼接 | 快速 | SQL 注入面，用户可以通过 config 注入任意 DDL | 永不（在 DDL 中）；在 DML 参数位置永远用 `?` 占位 |
| 排除过滤器复用 `#[deprecated]` 的 `should_keep()` | 少写代码 | 引入 OR/AND 语义错位，与热路径行为不一致 | 永不 |
| 不为排除过滤器添加 `validate_regexes()` 调用 | 少改代码 | 非法正则在运行时 panic（`expect("regex validated")` 在 `CompiledMetaFilters::from_meta` 中会触发） | 永不——必须在 validate 阶段覆盖所有新增的 regex 字段 |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| FILTER-03 + 预扫描两遍设计 | 排除过滤器影响 `has_transaction_filters()` 返回值，导致不必要的预扫描 | 排除过滤器是记录级，不触发预扫描；`has_transaction_filters()` 只在包含型事务过滤存在时返回 true |
| FILTER-03 + `pipeline.is_empty()` | 排除处理器被无条件添加 | 只有配置了非空排除规则时才添加处理器（见 Critical Pitfall 1） |
| PERF-10 + `parse_meta()` 共享 | 优化时重构 `process_with_meta` 签名 | 保留 `LogProcessor::process_with_meta` 的默认实现向后兼容，仅 override 热路径处理器 |
| DEBT-02 + config.apply_overrides | `apply_overrides` 中直接赋值 `table_name`（config.rs:162）不经过 validate | 在 `apply_overrides` 后必须重新调用 `validate()`，或在 `apply_one()` 中做内联校验 |
| PERF-11 + TOML 解析 | 用 `toml::from_str` 加载后立即调用计算密集型初始化 | 先 deserialize，validate，然后 lazy-init 昂贵资源（Regex 编译在 validate 后、使用前） |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| 每次 `process()` 调用都执行 `Regex::new()` | 处理速度从 5M/s 降到 <100k/s | 启动时预编译，存 `Vec<Regex>` 字段 | 任何数量级，即刻退化 |
| 排除过滤器向 `CompiledMetaFilters` 添加新 `Vec<Regex>` 字段但不更新 `has_filters()` | 有规则时 `has_filters()` 返回 false，跳过过滤 | 所有新字段必须纳入 `has_filters()` 检查 | 开始使用排除功能即触发 |
| 在 `SqlFilters::has_filters()` 中只检查 include，忽略 exclude | `exclude_patterns` 单独配置时 `has_filters()` 返回 false，过滤器不激活 | 已存在此 bug 的正确实现（两者 OR）；新增字段需跟进 | 纯排除配置时（无 include），全量记录通过 |
| TOML 配置反序列化用 `toml::from_str` + Regex 立即编译放在同一步 | 启动时间随规则数线性增长 | 反序列化（fast）和编译（slow）分离，compile 在 `FilterProcessor::new()` 中按需执行 | 规则数 >50 时启动延迟明显（>100ms） |
| `build_create_sql` / `build_insert_sql` 在 `initialize()` 中拼接 `table_name` | 若 `table_name` 含 SQLite 关键字（如 `order`）且未加引号，SQL parse 失败 | 白名单校验（只允许字母数字下划线）或始终加双引号转义 | 用户配置非标准表名时 |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| `format!("DROP TABLE IF EXISTS {}", table_name)` 直接拼接 | 用户通过 config 注入 DDL，如 `table_name = "t; DROP TABLE sqlite_master; --"` | 白名单校验（`[a-zA-Z0-9_]`）+ 双引号转义组合，在 config validate 阶段拒绝 |
| `format!("DELETE FROM {}", table_name)` | 同上 | 同上 |
| `build_insert_sql` 和 `build_create_sql` 中插值 `table_name` | 同上 | 同上；注意这三处都需要修复，不能只改一处 |

注：`INSERT` 语句的列值已全部使用 `?` 占位符，值注入风险已消除。风险集中在 DDL 中的标识符。

---

## "Looks Done But Isn't" Checklist

- [ ] **FILTER-03 排除过滤器：** 检查 `validate_regexes()` 是否覆盖了新增的排除字段，否则 `CompiledMetaFilters::from_meta()` 中的 `expect("regex validated")` 会在运行时 panic
- [ ] **FILTER-03 语义：** 有测试用例覆盖"排除规则 + 包含规则同时存在"的场景，且排除优先于包含
- [ ] **FILTER-03 快路径：** 无过滤配置时 `pipeline.is_empty()` 仍为 true（单元测试断言）
- [ ] **DEBT-02 table_name：** 全部三处 DDL 拼接（DROP/DELETE/CREATE/INSERT）都已修复，不仅仅是 INSERT
- [ ] **DEBT-01 静默错误：** `let _ =` 删除后，确认替代实现区分了"表不存在（无害）"和"真实错误（需 warn）"
- [ ] **PERF-10 热路径：** 排除过滤器加入后，无过滤配置的基准测试（`cargo criterion`）性能未退化
- [ ] **PERF-11 启动速度：** config 加载 + validate + pipeline 初始化总时间在 100ms 以内（在规则数 <20 的典型场景下）
- [ ] **向后兼容：** 现有无过滤配置的 TOML 文件（不含 `exclude_*` 字段）正常反序列化，651 个现有测试全部通过

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| 快路径被破坏，性能退化 | LOW | 在 `build_pipeline()` 加 `has_exclude_filters()` 守卫，删除空处理器；criterion 验证恢复 |
| 排除语义实现为 AND（应为 OR） | MEDIUM | 重写 `ExcludeFilterProcessor::matches()` 为 `any()`；补充测试矩阵；651 测试全跑 |
| table_name 注入修复过于严格，用户表名被拒 | LOW | 放宽正则（如允许 Unicode 字母），或支持 SQLite 双引号转义路径；config validate 给出清晰提示 |
| `let _ =` 删除后错误处理逻辑引入 regression | LOW | 按 rusqlite ErrorCode 细分；补充 integration test 覆盖"表不存在"和"删除失败"两个 case |
| Regex 预编译被遗漏，热路径动态编译 | MEDIUM | flamegraph 定位，将 `Regex::new()` 移至 `new()` 构造函数；criterion 验证恢复 |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 快路径被 FILTER-03 破坏 | FILTER-03 实现阶段 | `pipeline.is_empty()` 单元测试；cargo criterion 无退化 |
| 排除/包含语义混淆 | FILTER-03 实现阶段（设计先行） | 语义矩阵测试用例（4 个组合） |
| 排除过滤器触发不必要预扫描 | FILTER-03 实现阶段 | `has_transaction_filters()` 单元测试 |
| Regex 热路径重复编译 | FILTER-03 实现 + PERF-10 验证 | cargo criterion 基准对比；无排除规则时性能不退化 |
| table_name SQL 注入 | DEBT-02 修复阶段（最早） | 测试含特殊字符表名被 validate 拒绝；三处 DDL 全测试 |
| DELETE 静默错误 | DEBT-01 修复阶段（最早） | 模拟 DELETE 失败场景，验证 warn! 日志出现 |
| validate_regexes 漏掉新排除字段 | FILTER-03 实现阶段 | 配置非法正则的排除规则，期望 validate 返回 Err 而非 panic |
| 651 存量测试被破坏 | 所有实现阶段 | `cargo test` 全量通过作为每个 phase 的 exit criteria |

---

## Sources

- 直接代码检查：`src/features/filters.rs`（CompiledMetaFilters, CompiledSqlFilters, pipeline 快路径）
- 直接代码检查：`src/exporter/sqlite.rs`（DDL 拼接，`let _ =` 静默错误，line 265）
- 直接代码检查：`src/cli/run.rs`（build_pipeline, FilterProcessor, process_log_file 热循环）
- 直接代码检查：`src/config.rs`（table_name 现有校验只检查非空，line 397）
- Rust regex crate 文档：`Regex::new()` 的编译开销是匹配的数量级更高（DFA 构造）
- SQLite 官方文档：标识符不支持参数化，必须白名单或引号转义

---
*Pitfalls research for: sqllog2db v1.2 (FILTER-03, PERF-10, PERF-11, DEBT-01/02)*
*Researched: 2026-05-10*

# Phase 6: 解析库集成 + 验收 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-10
**Phase:** 6-parser-acceptance
**Areas discussed:** PERF-07 结论范围, 验收标准, 未提交变更处理

---

## PERF-07 结论范围

| 选项 | 说明 | 选中 |
|------|------|------|
| 关闭 — 自动生效，无需额外集成 | mmap + par_iter() 已在用，1.0.0 编码检测和 madvise 自动改善。index() 对流式写入无意义。记录调研结论后关闭。 | ✓ |
| 评估 index() 集成预扫描 | 用 index() 构建偏移列表，替换 par_iter() 的动态分块。需要实测对比，代码改动量中等。 | |

**用户选择：** 关闭，自动生效，无需额外集成
**Notes:** 侦察确认预扫描已调用 `parser.par_iter()`（0.9.1 即有），1.0.0 改进（tail 编码采样、madvise、小文件单分区）对现有代码自动生效，无需任何代码改动。

---

## 验收标准

| 选项 | 说明 | 选中 |
|------|------|------|
| cargo test + clippy + fmt 就够 | PERF-09 所有要求已覆盖。库的改进无降级风险，无需 criterion 验证。 | ✓ |
| 加跑 criterion 对比 | 运行合成 benchmark 对比 Phase 3/4/5 基准，确认 1.0.0 升级无性能退化。耗时较长。 | |

**用户选择：** cargo test + clippy + fmt 就够
**Notes:** 不跑 benchmark，降低 Phase 6 执行复杂度。

---

## 未提交变更处理

| 选项 | 说明 | 选中 |
|------|------|------|
| 统一纳入 Phase 6 提交 | 将 Cargo.toml 升级 + Phase 4/5 遗留变更一起提交，Phase 6 有一个干净的 git 历史。 | ✓ |
| 区分提交 | 先单独提交 Phase 4/5 遗留变更，再提交 Phase 6 的 Cargo 升级。历史更清晰但需额外操作。 | |

**用户选择：** 统一纳入 Phase 6 提交
**Notes:** 遗留变更包括 config.toml、benchmark estimates、04-csv review 文件、Phase 5 新基线目录。

---

## Claude's Discretion

无 — 用户对所有三个灰区都给出了明确选择。

## Deferred Ideas

- `index()` / `RecordIndex` 两阶段并行扫描 — 不适用当前场景，未来有需要时重新评估
- criterion 验证 1.0.0 升级无退化 — 理论无风险，如有疑虑可在 CI 补充

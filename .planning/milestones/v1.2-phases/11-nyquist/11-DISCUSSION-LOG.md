# Phase 11: Nyquist 补签 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-15
**Phase:** 11-nyquist
**Areas discussed:** 更新深度（Phases 3/4/5）, Phase 6 内容结构, Phase 5 WAL 测试问题

---

## 更新深度（Phases 3/4/5）

| Option | Description | Selected |
|--------|-------------|----------|
| 全量回溯 | 更新每个任务 Status（✅ green）、Wave 0 逐项勾选、sign-off 全勾、nyquist_compliant: true、status: complete、Approval: signed | ✓ |
| 轻量签署 | 只改 frontmatter 和 sign-off，保留任务状态为原始计划快照 | |
| 介于二者 | 勾选 Wave 0 和 sign-off，任务 Status 保留原样 | |

**User's choice:** 全量回溯
**Notes:** 证据来源用 VERIFICATION.md + SUMMARY.md 两者组合。

---

## 证据来源

| Option | Description | Selected |
|--------|-------------|----------|
| VERIFICATION.md + SUMMARY.md | 从验证报告和执行摘要共同提取证据 | ✓ |
| 仅 VERIFICATION.md | 只看最终验证报告 | |
| 不写具体证据 | 只更新状态标志，不附证据 | |

**User's choice:** VERIFICATION.md + SUMMARY.md

---

## Phase 6 内容结构

| Option | Description | Selected |
|--------|-------------|----------|
| 对齐 Phase 3-5 结构 | 完整的标准格式：frontmatter + 测试基础设施 + 采样频率 + 每任务验证映射 + sign-off | ✓ |
| 更精简的回溯格式 | 只含 frontmatter + 任务映射 + sign-off，省略测试基础设施表和采样频率节 | |

**User's choice:** 对齐 Phase 3-5 结构
**Notes:** Plan 01 对应 Wave 1（依赖升级 + PERF-07 调研），Plan 02 对应 Wave 2（验收测试）。所有任务直接标记 ✅ green。

---

## Phase 5 WAL 测试问题

| Option | Description | Selected |
|--------|-------------|----------|
| 标记为 N/A 并加注说明 | WAL 相关任务和 Wave 0 项标注 N/A + 移除原因，sign-off 仍全部通过 | ✓ |
| 直接删除 WAL 相关行 | 从任务映射和 Wave 0 中删去 WAL 条目 | |
| 保留原样不处理 | 不动 WAL 项，只更新 frontmatter 和 sign-off | |

**User's choice:** 标记为 N/A 并加注说明
**Notes:** 注释模板："N/A — 用户决策移除（PERF-05 canceled）：数据无需崩溃保护，保留 JOURNAL_MODE=OFF SYNCHRONOUS=OFF 高性能模式。见 05-VERIFICATION.md §Re-verification Context。"

---

## Claude's Discretion

- Approval 行的附注文字措辞
- Phase 6 测试基础设施描述的详细程度

## Deferred Ideas

None — 讨论完全在 Phase 11 范围内进行。

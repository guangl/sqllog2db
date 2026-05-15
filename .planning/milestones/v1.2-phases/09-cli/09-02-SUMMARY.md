---
phase: 09-cli
plan: "02"
subsystem: cli
tags: [rust, threading, self-update, fire-and-forget]

# Dependency graph
requires: []
provides:
  - "check_for_updates_at_startup 后台化：网络 I/O 在 thread::spawn 闭包中执行，主流程不再阻塞"
affects: [09-cli]

# Tech tracking
tech-stack:
  added: []
  patterns: ["thread::spawn fire-and-forget 模式：不保留 JoinHandle，主流程立即返回"]

key-files:
  created: []
  modified:
    - src/cli/update.rs

key-decisions:
  - "JoinHandle 直接丢弃，不调用 .join()，实现 fire-and-forget 语义（per D-05）"
  - "warn!() 保留在闭包内，通过 env_logger 输出到 stderr，与主流程输出交错属已接受行为（per D-06）"

patterns-established:
  - "fire-and-forget 后台检查：thread::spawn + 丢弃 JoinHandle，适用于非关键后台任务"

requirements-completed: [PERF-11]

# Metrics
duration: 8min
completed: "2026-05-14"
---

# Phase 09 Plan 02: check_for_updates_at_startup 后台化 Summary

**将 GitHub API 调用移入 std::thread::spawn 闭包，主流程启动时不再阻塞于网络等待，fire-and-forget 语义**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-14T01:34:00Z
- **Completed:** 2026-05-14T01:42:27Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- `check_for_updates_at_startup` 函数体整体移入 `std::thread::spawn(|| { ... })` 闭包
- JoinHandle 直接丢弃，主流程调用后立即返回，GitHub API 调用在后台完成
- 函数签名 `pub fn check_for_updates_at_startup()` 完全保持不变，无需修改任何调用方
- cargo build --release / clippy / test（50 passed, 0 failed）全部通过

## Task Commits

每个任务原子提交：

1. **Task 1: 将 check_for_updates_at_startup 网络逻辑移入 thread::spawn** - `b407d02` (perf)

## Files Created/Modified

- `src/cli/update.rs` — 将 check_for_updates_at_startup 函数体移入 thread::spawn 闭包，fire-and-forget

## Decisions Made

- JoinHandle 直接丢弃（不赋值给变量），实现 fire-and-forget 语义，符合 D-05 决策
- warn!() 调用位置不变（仍在闭包内），通过 env_logger 输出到 stderr；输出与主流程可能交错，属已接受行为（D-06）

## Deviations from Plan

无 — 计划完全按照规格执行。

## Issues Encountered

无。

## Threat Flags

无新增安全相关表面。

## Known Stubs

无。

## Self-Check

- [x] `src/cli/update.rs` 存在且已修改（commit b407d02）
- [x] `grep -n "thread::spawn" src/cli/update.rs` 返回 1 行（L68）
- [x] `grep -n "fn check_for_updates_at_startup" src/cli/update.rs` 返回 1 行，签名不变（L67）
- [x] `grep -n "JoinHandle\|\.join()" src/cli/update.rs` 返回 0 行（仅注释）
- [x] cargo build --release 通过
- [x] cargo clippy --all-targets -- -D warnings 通过
- [x] cargo test：50 passed, 0 failed

## Self-Check: PASSED

## Next Phase Readiness

- 09-02 完成：check_for_updates_at_startup 已后台化，主流程启动速度不再受网络延迟影响
- 09-03（配置加载提速）和 09-04（热路径优化）可以继续推进

---
*Phase: 09-cli*
*Completed: 2026-05-14*

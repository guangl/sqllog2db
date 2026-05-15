---
plan: 12-01
phase: 12-sql
status: completed
commit: e88cd4f
---

# Plan 12-01: sql_fingerprint 共享扫描引擎 + normalize_template

## 完成内容

- 将 `NEEDS_SPECIAL` 重命名为 `NEEDS_SPECIAL_NORM`，扩展加入 `-` 和 `/`（用于注释检测）
- 新增 `#[derive(Clone, Copy)] enum ScanMode { Fingerprint, Normalize }`
- 抽取私有 `fn scan_sql_bytes(sql: &str, mode: ScanMode) -> String` 共享扫描引擎
- `pub fn fingerprint()` 重构为 `scan_sql_bytes(sql, ScanMode::Fingerprint)` 薄包装（签名不变，行为零回归）
- 新增 `pub fn normalize_template()` → `scan_sql_bytes(sql, ScanMode::Normalize)`
- 辅助函数：`dispatch_byte`, `handle_quote`, `handle_line_comment`, `handle_block_comment`, `handle_word`, `try_fold_in_list`, `skip_quoted`, `is_subquery`, `is_keyword`, `is_ident_byte`
- 新增 8 项 normalize_template 测试（注释去除、IN 折叠、关键字大写、字面量保护、空白折叠等）
- 原 9 项 fingerprint 测试零回归

## 验收通过

- 17 项单元测试全绿（9 原 fingerprint + 8 normalize_template）
- `cargo clippy --all-targets -- -D warnings` 零 warning
- `cargo test` 全套 50 项通过

---
status: complete
phase: 02-shuchu-ziduan-kongzhi
source:
  - 02-01-SUMMARY.md
  - 02-02-SUMMARY.md
  - 02-03-SUMMARY.md
  - 02-04-SUMMARY.md
started: "2026-04-18T00:00:00.000Z"
updated: "2026-04-18T00:01:00.000Z"
---

## Current Test

[testing complete]

## Tests

### 1. 默认行为不变（不配置 fields）
expected: config.toml 中不配置 fields 时（或注释掉 fields 行），运行工具后 CSV 输出包含所有字段列，与未修改前完全相同，无任何字段缺失。
result: pass

### 2. CSV 字段过滤
expected: |
  在 config.toml [features] 中添加 fields = ["sql_text", "user_name"]（或类似两个有效字段），
  运行后 CSV 只包含这两列，其余列完全不出现在输出中。
result: pass

### 3. CSV 字段顺序与配置一致
expected: |
  配置 fields = ["schema_name", "sql_text", "user_name"]（故意与默认列顺序不同），
  运行后 CSV 表头第一列是 schema_name，第二列是 sql_text，第三列是 user_name，
  顺序与配置中的 fields 数组顺序完全一致。
result: pass

### 4. SQLite 字段过滤
expected: |
  配置 fields 列表后，切换到 SQLite 导出（[exporter.sqlite] 配置），
  运行后 SQLite 数据库表中只有指定的列，CREATE TABLE 语句只含配置的字段。
result: pass

### 5. 无效字段名启动报错
expected: |
  在 fields 中添加一个不存在的字段名（如 fields = ["sql_text", "nonexistent_field"]），
  运行 `cargo run -- run -c config.toml` 时，程序在启动阶段（处理任何记录之前）打印错误信息，
  提示该字段名无效，然后退出，而不是静默忽略或在运行时崩溃。
result: pass

## Summary

total: 5
passed: 5
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]

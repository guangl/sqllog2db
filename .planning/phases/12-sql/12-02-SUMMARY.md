---
plan: 12-02
phase: 12-sql
status: completed
commit: bb43674
---

# Plan 12-02: TemplateAnalysisConfig + pub use normalize_template + init 模板

## 完成内容

- `src/features/mod.rs` 新增 `pub use sql_fingerprint::normalize_template` 导出（D-09）
- 新增 `TemplateAnalysisConfig { enabled: bool }` 结构体（`#[serde(default)]`，默认 false，D-10/D-12）
- `FeaturesConfig` 新增 `template_analysis: Option<TemplateAnalysisConfig>` 字段（D-11）
- `src/cli/init.rs` 中英文 TOML 模板各新增 `[features.template_analysis]` 段（含注释 + `enabled = false`）
- 修复 `show_config.rs` 中的结构体字面量（补充 `template_analysis: None` 字段）
- 新增 3 项配置测试：`test_template_analysis_config_default`、`test_template_analysis_config_deserialize_enabled_true`、`test_template_analysis_config_deserialize_empty_is_false`

## 验收通过

- 18 项 features 单元测试全绿
- `cargo clippy --all-targets -- -D warnings` 零 warning
- `cargo test` 全套 50 项通过

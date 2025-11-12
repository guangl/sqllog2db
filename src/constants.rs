//! 公共常量与复用函数
//! 提供：
//! - 合法日志级别常量 LOG_LEVELS
//! - 表结构字段模板 DB_TABLE_COLUMNS_SQL

/// 合法的日志级别（统一来源）
pub const LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];

//! 公共常量与复用函数
//! 提供：
//! - 合法日志级别常量 `LOG_LEVELS`
//! - 数据库表结构与 SQL 语句模板

/// 合法的日志级别（统一来源）
pub const LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];

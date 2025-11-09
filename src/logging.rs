use crate::config::LoggingConfig;
use crate::constants::LOG_LEVELS;
use crate::error::{Error, FileError, Result};
use std::path::Path;
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// 初始化日志系统
pub fn init_logging(config: &LoggingConfig) -> Result<()> {
    // 解析日志级别
    let level = parse_log_level(&config.level)?;

    // 获取日志文件路径和目录
    let log_path = Path::new(&config.path);
    let parent_dir = log_path.parent().ok_or_else(|| {
        Error::File(FileError::CreateDirectoryFailed {
            path: log_path.to_path_buf(),
            reason: "无法获取父目录".to_string(),
        })
    })?;

    // 创建日志目录（如果不存在）
    if !parent_dir.exists() {
        std::fs::create_dir_all(parent_dir).map_err(|e| {
            Error::File(FileError::CreateDirectoryFailed {
                path: parent_dir.to_path_buf(),
                reason: e.to_string(),
            })
        })?;
    }

    // 从路径中提取基础文件名（去掉扩展名）
    let file_stem = log_path
        .file_stem()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            Error::File(FileError::CreateDirectoryFailed {
                path: log_path.to_path_buf(),
                reason: "无效的文件名".to_string(),
            })
        })?;

    let extension = log_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("log");

    // 创建文件 appender（每天滚动，自动管理保留天数）
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(file_stem)
        .filename_suffix(extension)
        .max_log_files(config.retention_days())
        .build(parent_dir)
        .map_err(|e| {
            Error::File(FileError::CreateDirectoryFailed {
                path: parent_dir.to_path_buf(),
                reason: e.to_string(),
            })
        })?;

    // 使用 LevelFilter 代替 EnvFilter（移除 env-filter 特性以减小体积）
    let subscriber = tracing_subscriber::registry()
        .with(LevelFilter::from_level(level))
        .with(
            fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_line_number(true),
        )
        .with(fmt::layer().with_writer(std::io::stdout).with_target(false));

    subscriber.init();

    tracing::info!(
        "日志系统初始化完成 - 级别: {}, 文件: {}, 保留天数: {}",
        level.as_str(),
        config.path,
        config.retention_days()
    );

    Ok(())
}

/// 解析日志级别字符串
fn parse_log_level(level_str: &str) -> Result<Level> {
    let lower = level_str.to_lowercase();
    let mapped = match lower.as_str() {
        "trace" => Some(Level::TRACE),
        "debug" => Some(Level::DEBUG),
        "info" => Some(Level::INFO),
        "warn" => Some(Level::WARN),
        "error" => Some(Level::ERROR),
        _ => None,
    };
    mapped.ok_or_else(|| {
        Error::Config(crate::error::ConfigError::InvalidLogLevel {
            level: level_str.to_string(),
            valid_levels: LOG_LEVELS.iter().map(|s| s.to_string()).collect(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_level_valid() {
        assert_eq!(parse_log_level("trace").unwrap(), Level::TRACE);
        assert_eq!(parse_log_level("debug").unwrap(), Level::DEBUG);
        assert_eq!(parse_log_level("info").unwrap(), Level::INFO);
        assert_eq!(parse_log_level("warn").unwrap(), Level::WARN);
        assert_eq!(parse_log_level("error").unwrap(), Level::ERROR);
    }

    #[test]
    fn test_parse_log_level_case_insensitive() {
        assert_eq!(parse_log_level("INFO").unwrap(), Level::INFO);
        assert_eq!(parse_log_level("Debug").unwrap(), Level::DEBUG);
        assert_eq!(parse_log_level("WARN").unwrap(), Level::WARN);
    }

    #[test]
    fn test_parse_log_level_invalid() {
        assert!(parse_log_level("invalid").is_err());
        assert!(parse_log_level("critical").is_err());
    }

    // 注意：init_logging() 使用 tracing::subscriber::set_global_default
    // 只能调用一次，因此无法在单元测试中多次测试初始化逻辑
    // 目录创建逻辑在实际运行时已验证，这里仅测试辅助函数
}

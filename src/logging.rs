use crate::config::LoggingConfig;
use crate::constants::LOG_LEVELS;
use crate::error::{Error, FileError, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

// 使用 once_cell 缓存日志级别映射表，避免每次查找时重新构建
static LOG_LEVEL_MAP: Lazy<HashMap<&'static str, Level>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("trace", Level::TRACE);
    map.insert("debug", Level::DEBUG);
    map.insert("info", Level::INFO);
    map.insert("warn", Level::WARN);
    map.insert("error", Level::ERROR);
    map
});

/// 初始化日志系统
pub fn init_logging(config: &LoggingConfig) -> Result<()> {
    // 解析日志级别
    let level = parse_log_level(&config.level)?;

    // 获取日志文件路径和目录
    let log_path = Path::new(&config.file);
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
        config.file,
        config.retention_days()
    );

    Ok(())
}

/// 解析日志级别字符串
fn parse_log_level(level_str: &str) -> Result<Level> {
    let lower = level_str.to_lowercase();
    LOG_LEVEL_MAP.get(lower.as_str()).copied().ok_or_else(|| {
        Error::Config(crate::error::ConfigError::InvalidLogLevel {
            level: level_str.to_string(),
            valid_levels: LOG_LEVELS.iter().map(|s| s.to_string()).collect(),
        })
    })
}

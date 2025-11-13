use crate::config::LoggingConfig;
use crate::constants::LOG_LEVELS;
use crate::error::{Error, FileError, Result};
use log::SetLoggerError;
use log::{Level, LevelFilter, Metadata, Record};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

// 使用 once_cell 缓存日志级别映射表，避免每次查找时重新构建
static LOG_LEVEL_MAP: Lazy<HashMap<&'static str, LevelFilter>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("trace", LevelFilter::Trace);
    map.insert("debug", LevelFilter::Debug);
    map.insert("info", LevelFilter::Info);
    map.insert("warn", LevelFilter::Warn);
    map.insert("error", LevelFilter::Error);
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

    // 创建简单的追加日志文件（不做滚动），更轻量：使用 Arc<Mutex<File>> 作为共享 writer
    let log_file_path = parent_dir.join(format!("{}.{}", file_stem, extension));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .map_err(|e| {
            Error::File(FileError::CreateDirectoryFailed {
                path: log_file_path.clone(),
                reason: e.to_string(),
            })
        })?;

    let shared_file = Arc::new(Mutex::new(file));

    // 自定义简单 Logger，写入文件与 stdout
    struct SimpleLogger {
        level: LevelFilter,
        file: Arc<Mutex<std::fs::File>>,
    }

    impl log::Log for SimpleLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            match self.level {
                LevelFilter::Off => false,
                LevelFilter::Error => metadata.level() == Level::Error,
                LevelFilter::Warn => metadata.level() <= Level::Warn,
                LevelFilter::Info => metadata.level() <= Level::Info,
                LevelFilter::Debug => metadata.level() <= Level::Debug,
                LevelFilter::Trace => true,
            }
        }

        fn log(&self, record: &Record) {
            if !self.enabled(record.metadata()) {
                return;
            }

            let msg = format!(
                "[{}] {} - {}\n",
                record.level(),
                record.target(),
                record.args()
            );

            // 写到 stdout
            let _ = std::io::stdout().write_all(msg.as_bytes());

            // 写到文件
            if let Ok(mut f) = self.file.lock() {
                let _ = f.write_all(msg.as_bytes());
            }
        }

        fn flush(&self) {}
    }

    let logger = SimpleLogger {
        level,
        file: shared_file.clone(),
    };

    // 注册 logger
    log::set_max_level(level);
    log::set_boxed_logger(Box::new(logger)).map_err(|e: SetLoggerError| {
        Error::File(FileError::CreateDirectoryFailed {
            path: log_file_path.clone(),
            reason: format!("设置日志器失败: {}", e),
        })
    })?;

    log::info!(
        "日志系统初始化完成 - 级别: {:?}, 文件: {}, 保留天数: {}",
        level,
        config.file,
        config.retention_days()
    );

    Ok(())
}

/// 解析日志级别字符串
fn parse_log_level(level_str: &str) -> Result<LevelFilter> {
    let lower = level_str.to_lowercase();
    LOG_LEVEL_MAP.get(lower.as_str()).copied().ok_or_else(|| {
        Error::Config(crate::error::ConfigError::InvalidLogLevel {
            level: level_str.to_string(),
            valid_levels: LOG_LEVELS.iter().map(|s| s.to_string()).collect(),
        })
    })
}

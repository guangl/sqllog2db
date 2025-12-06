use crate::config::LoggingConfig;
use crate::constants::LOG_LEVELS;
use crate::error::{Error, FileError, Result};
use chrono::Local;
use log::SetLoggerError;
use log::{Level, LevelFilter, Metadata, Record};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

// 使用 LazyLock 缓存日志级别映射表，避免每次查找时重新构建
static LOG_LEVEL_MAP: LazyLock<HashMap<&'static str, LevelFilter>> = LazyLock::new(|| {
    let mut map = HashMap::with_capacity(5);
    map.insert("trace", LevelFilter::Trace);
    map.insert("debug", LevelFilter::Debug);
    map.insert("info", LevelFilter::Info);
    map.insert("warn", LevelFilter::Warn);
    map.insert("error", LevelFilter::Error);
    map
});

/// 日志模式：是否输出到控制台
static LOG_TO_CONSOLE: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(true));

/// 设置日志是否输出到控制台
pub fn set_log_to_console(enabled: bool) {
    if let Ok(mut console_enabled) = LOG_TO_CONSOLE.lock() {
        *console_enabled = enabled;
    }
}

/// 初始化日志系统
pub fn init_logging(config: &LoggingConfig) -> Result<()> {
    // 解析日志级别
    let level = parse_log_level(&config.level)?;

    // 获取日志文件路径和目录
    let log_path = Path::new(&config.file);
    let parent_dir = log_path.parent().ok_or_else(|| {
        Error::File(FileError::CreateDirectoryFailed {
            path: log_path.to_path_buf(),
            reason: "Failed to get parent directory".to_string(),
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
                reason: "Invalid filename".to_string(),
            })
        })?;

    let extension = log_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("log");

    // 创建简单的追加日志文件（不做滚动），更轻量：使用 Arc<Mutex<File>> 作为共享 writer
    let log_file_path = parent_dir.join(format!("{file_stem}.{extension}"));
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
            let now = Local::now().format("%Y-%m-%d %H:%M:%S");
            let msg = format!(
                "[{}][{}] {} - {}\n",
                now,
                record.level(),
                record.target(),
                record.args()
            );
            // 如果启用控制台输出，则写到 stdout
            if let Ok(console_enabled) = LOG_TO_CONSOLE.lock() {
                if *console_enabled {
                    let _ = std::io::stdout().write_all(msg.as_bytes());
                }
            }
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
            path: log_file_path,
            reason: format!("Failed to set logger: {e}"),
        })
    })?;

    log::info!(
        "Logging initialized - level: {:?}, file: {}, retention_days: {}",
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
            valid_levels: LOG_LEVELS.iter().map(|s| (*s).to_string()).collect(),
        })
    })
}

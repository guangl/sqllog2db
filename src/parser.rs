/// SQL 日志解析模块
/// 使用 dm-database-parser-sqllog 库解析达梦数据库的 SQL 日志文件
use crate::error::{Error, ParserError, Result};
use log::{debug, info, warn};
use std::path::{Path, PathBuf};

/// SQL 日志解析器
#[derive(Debug)]
pub struct SqllogParser {
    /// 日志路径（文件或目录）
    path: PathBuf,
}

impl SqllogParser {
    /// 创建新的 SQL 日志解析器
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// 获取日志路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 返回所有日志文件的路径列表
    /// 这样用户可以遍历文件，然后对每个文件使用 iter_sqllogs_from_file
    pub fn log_files(&self) -> Result<Vec<PathBuf>> {
        self.scan_log_files()
    }

    /// 扫描并获取所有需要解析的日志文件
    fn scan_log_files(&self) -> Result<Vec<PathBuf>> {
        let path = &self.path;

        if !path.exists() {
            return Err(Error::Parser(ParserError::PathNotFound {
                path: path.clone(),
            }));
        }

        let mut log_files = Vec::new();

        if path.is_file() {
            // 单个文件
            info!("Parsing single log file: {}", path.display());
            log_files.push(path.clone());
        } else if path.is_dir() {
            // 目录：扫描所有 .log 文件
            info!("Scanning log directory: {}", path.display());

            let entries = std::fs::read_dir(path).map_err(|e| {
                Error::Parser(ParserError::ReadDirFailed {
                    path: path.clone(),
                    reason: e.to_string(),
                })
            })?;

            for entry in entries {
                let entry = entry.map_err(|e| {
                    Error::Parser(ParserError::ReadDirFailed {
                        path: path.clone(),
                        reason: e.to_string(),
                    })
                })?;

                let entry_path = entry.path();

                // 只处理 .log 文件
                if entry_path.is_file() && entry_path.extension().is_some_and(|ext| ext == "log") {
                    debug!("Found log file: {}", entry_path.display());
                    log_files.push(entry_path);
                }
            }

            if log_files.is_empty() {
                warn!("No .log files found in directory {}", path.display());
            } else {
                info!("Found {} log files", log_files.len());
            }
        } else {
            return Err(Error::Parser(ParserError::InvalidPath {
                path: path.clone(),
                reason: "既不是文件也不是目录".to_string(),
            }));
        }

        Ok(log_files)
    }
}

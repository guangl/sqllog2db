/// SQL 日志解析模块
/// 使用 dm-database-parser-sqllog 库解析达梦数据库的 SQL 日志文件
use crate::error::{Error, ParserError, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// SQL 日志解析器
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
            info!("解析单个日志文件: {}", path.display());
            log_files.push(path.clone());
        } else if path.is_dir() {
            // 目录：扫描所有 .log 文件
            info!("扫描日志目录: {}", path.display());

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
                if entry_path.is_file() {
                    if let Some(ext) = entry_path.extension() {
                        if ext == "log" {
                            debug!("发现日志文件: {}", entry_path.display());
                            log_files.push(entry_path);
                        }
                    }
                }
            }

            if log_files.is_empty() {
                warn!("目录 {} 中没有找到 .log 文件", path.display());
            } else {
                info!("找到 {} 个日志文件", log_files.len());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn create_test_log_file(path: &Path, content: &str) {
        let mut file = fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_parser_new() {
        let parser = SqllogParser::new("test.log");
        assert_eq!(parser.path(), Path::new("test.log"));
    }

    #[test]
    fn test_scan_log_files_nonexistent_path() {
        let parser = SqllogParser::new("nonexistent_path_12345");
        let result = parser.scan_log_files();
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.to_string().contains("路径不存在"));
        }
    }

    #[test]
    fn test_scan_log_files_single_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_scan_single.log");

        create_test_log_file(&test_file, "test content");

        let parser = SqllogParser::new(&test_file);
        let result = parser.scan_log_files();

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], test_file);

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_scan_log_files_directory() {
        let temp_dir = std::env::temp_dir().join("test_scan_dir");
        let _ = fs::create_dir(&temp_dir);

        // 创建几个测试文件
        create_test_log_file(&temp_dir.join("file1.log"), "log 1");
        create_test_log_file(&temp_dir.join("file2.log"), "log 2");
        create_test_log_file(&temp_dir.join("file3.txt"), "not a log");

        let parser = SqllogParser::new(&temp_dir);
        let result = parser.scan_log_files();

        assert!(result.is_ok());
        let files = result.unwrap();

        // 应该只找到 2 个 .log 文件
        assert_eq!(files.len(), 2);

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scan_log_files_empty_directory() {
        let temp_dir = std::env::temp_dir().join("test_scan_empty");
        let _ = fs::create_dir(&temp_dir);

        let parser = SqllogParser::new(&temp_dir);
        let result = parser.scan_log_files();

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 0);

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // 注意：log_files() 方法只返回文件列表，实际解析由调用者使用 iter_sqllogs_from_file 完成
    #[test]
    fn test_log_files_nonexistent_path() {
        let parser = SqllogParser::new("nonexistent_path_99999");
        let result = parser.log_files();
        // 不存在的路径会产生错误
        assert!(result.is_err());
    }

    #[test]
    fn test_log_files_empty_directory() {
        let temp_dir = std::env::temp_dir().join("test_log_files_empty");
        let _ = fs::create_dir(&temp_dir);

        let parser = SqllogParser::new(&temp_dir);
        let result = parser.log_files();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }
}

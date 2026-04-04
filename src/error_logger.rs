use crate::error::{Error, ExportError, Result};
use log::{debug, info};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// 将解析失败的原始数据记录到文件
pub struct ErrorLogger {
    writer: BufWriter<File>,
    path: PathBuf,
    count: usize,
}

impl std::fmt::Debug for ErrorLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorLogger")
            .field("path", &self.path)
            .field("count", &self.count)
            .finish_non_exhaustive()
    }
}

impl ErrorLogger {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        if let Some(parent) = path.parent().filter(|p| !p.exists()) {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Export(ExportError::WriteError {
                    path: parent.to_path_buf(),
                    reason: e.to_string(),
                })
            })?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| {
                Error::Export(ExportError::WriteError {
                    path: path.clone(),
                    reason: e.to_string(),
                })
            })?;

        info!("Error logger initialized: {}", path.display());

        Ok(Self {
            writer: BufWriter::new(file),
            path,
            count: 0,
        })
    }

    /// 记录来自 dm-database-parser-sqllog 的解析错误（格式：file | error | line）
    pub fn log_parse_error(
        &mut self,
        file_path: &str,
        error: &dm_database_parser_sqllog::ParseError,
    ) -> Result<()> {
        writeln!(self.writer, "{file_path} | {error:?}").map_err(|e| {
            Error::Export(ExportError::WriteError {
                path: self.path.clone(),
                reason: e.to_string(),
            })
        })?;
        self.count += 1;
        Ok(())
    }

    /// 返回已记录的解析错误数量
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.count
    }

    pub fn finalize(&mut self) -> Result<()> {
        self.writer.flush().map_err(|e| {
            Error::Export(ExportError::WriteError {
                path: self.path.clone(),
                reason: format!("flush failed: {e}"),
            })
        })?;
        if self.count > 0 {
            info!(
                "Error log: {} ({} records)",
                self.path.display(),
                self.count
            );
        } else {
            debug!("No parse errors");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("errors.log");
        let logger = ErrorLogger::new(&path).unwrap();
        assert_eq!(logger.error_count(), 0);
        assert!(path.exists());
    }

    #[test]
    fn test_new_creates_parent_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("subdir/nested/errors.log");
        let _logger = ErrorLogger::new(&path).unwrap();
        assert!(path.parent().unwrap().exists());
    }

    #[test]
    fn test_finalize_with_no_errors() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("errors.log");
        let mut logger = ErrorLogger::new(&path).unwrap();
        logger.finalize().unwrap();
        assert_eq!(logger.error_count(), 0);
    }

    #[test]
    fn test_log_parse_error_increments_count() {
        use dm_database_parser_sqllog::LogParser;
        let dir = tempfile::TempDir::new().unwrap();

        // Write a file with an invalid log line to provoke a ParseError
        let log_path = dir.path().join("bad.log");
        std::fs::write(&log_path, "not a valid log line at all\n").unwrap();

        let err_path = dir.path().join("errors.log");
        let mut logger = ErrorLogger::new(&err_path).unwrap();

        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        let mut got_error = false;
        for result in parser.iter() {
            if let Err(e) = result {
                logger
                    .log_parse_error(log_path.to_str().unwrap(), &e)
                    .unwrap();
                got_error = true;
                break;
            }
        }

        if got_error {
            assert_eq!(logger.error_count(), 1);
            logger.finalize().unwrap();
            let content = std::fs::read_to_string(&err_path).unwrap();
            assert!(!content.is_empty());
        }
    }

    #[test]
    fn test_finalize_with_errors_flushes() {
        use dm_database_parser_sqllog::LogParser;
        let dir = tempfile::TempDir::new().unwrap();

        let log_path = dir.path().join("bad2.log");
        std::fs::write(&log_path, "garbage\nmore garbage\n").unwrap();

        let err_path = dir.path().join("errors2.log");
        let mut logger = ErrorLogger::new(&err_path).unwrap();

        let parser = LogParser::from_path(log_path.to_str().unwrap()).unwrap();
        for result in parser.iter() {
            if let Err(e) = result {
                let _ = logger.log_parse_error(log_path.to_str().unwrap(), &e);
            }
        }

        logger.finalize().unwrap();
        // File should exist and be readable regardless of error count
        assert!(err_path.exists());
    }

    #[test]
    fn test_debug_format() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("errors.log");
        let logger = ErrorLogger::new(&path).unwrap();
        let debug_str = format!("{logger:?}");
        assert!(debug_str.contains("ErrorLogger"));
    }
}

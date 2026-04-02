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

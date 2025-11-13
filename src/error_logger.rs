/// 错误日志记录器 - 将解析失败的原始数据记录到文件
use crate::error::{Error, ExportError, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use tracing::{debug, info};

/// 解析错误记录（JSONL 格式）
#[derive(Debug, Serialize)]
pub struct ParseErrorRecord {
    /// 时间戳
    pub timestamp: String,
    /// 错误发生的文件路径
    pub file_path: String,
    /// 错误原因/描述
    pub error_message: String,
    /// 原始数据内容（导致解析失败的行或片段）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_content: Option<String>,
    /// 行号（如果适用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<usize>,
}

/// 错误日志记录器
#[derive(Debug, Default, Serialize)]
pub struct ErrorMetrics {
    /// 总错误数
    pub total: usize,
    /// 按分类统计
    pub by_category: HashMap<String, usize>,
    /// 解析错误的细分（变体）统计
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub parse_variants: HashMap<String, usize>,
}

impl ErrorMetrics {
    fn incr_category(&mut self, cat: &str) {
        *self.by_category.entry(cat.to_string()).or_insert(0) += 1;
        self.total += 1;
    }

    fn incr_parse_variant(&mut self, variant: &str) {
        *self.parse_variants.entry(variant.to_string()).or_insert(0) += 1;
    }
}

pub struct ErrorLogger {
    writer: BufWriter<File>,
    path: String,
    count: usize,
    metrics: ErrorMetrics,
    summary_path: String,
}

impl ErrorLogger {
    /// 创建新的错误日志记录器
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy().to_string();

        // 创建父目录（如果不存在）
        if let Some(parent) = path_ref.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    Error::Export(ExportError::FileCreateFailed {
                        path: parent.to_path_buf(),
                        reason: e.to_string(),
                    })
                })?;
            }
        }

        // 打开或创建文件（追加模式）
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path_ref)
            .map_err(|e| {
                Error::Export(ExportError::FileCreateFailed {
                    path: path_ref.to_path_buf(),
                    reason: e.to_string(),
                })
            })?;

        info!("错误日志记录器已初始化: {}", path_str);

        // summary 文件路径：errors.jsonl => errors.summary.json
        let summary_path = if let Some(stripped) = path_str.strip_suffix(".jsonl") {
            format!("{}.summary.json", stripped)
        } else {
            format!("{}.summary.json", path_str)
        };

        Ok(Self {
            writer: BufWriter::new(file),
            path: path_str,
            count: 0,
            metrics: ErrorMetrics::default(),
            summary_path,
        })
    }

    /// 记录一个解析错误
    pub fn log_error(&mut self, record: ParseErrorRecord) -> Result<()> {
        let json = serde_json::to_string(&record).map_err(|e| {
            Error::Export(ExportError::SerializationFailed {
                data_type: "ParseErrorRecord".to_string(),
                reason: e.to_string(),
            })
        })?;

        writeln!(self.writer, "{}", json).map_err(|e| {
            Error::Export(ExportError::FileWriteFailed {
                path: self.path.clone(),
                reason: e.to_string(),
            })
        })?;

        self.count += 1;
        // 记录分类统计（默认按 parse 分类，若调用方希望其它分类应使用 log_app_error）
        self.metrics.incr_category("parse");
        Ok(())
    }

    /// 记录来自 dm-database-parser-sqllog 的解析错误
    pub fn log_parse_error(
        &mut self,
        file_path: &str,
        error: &dm_database_parser_sqllog::ParseError,
    ) -> Result<()> {
        let record = ParseErrorRecord {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            file_path: file_path.to_string(),
            error_message: format!("{:?}", error),
            raw_content: None, // dm-database-parser-sqllog 的 ParseError 不包含原始内容
            line_number: None,
        };
        // 粗略使用 Debug 字符串作为 variant 标识
        let variant = format!("{:?}", error);
        self.metrics.incr_parse_variant(&variant);
        self.log_error(record)
    }

    /// 完成错误记录并生成 summary.json
    /// 刷新缓冲区
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush().map_err(|e| {
            Error::Export(ExportError::FileWriteFailed {
                path: self.path.clone(),
                reason: format!("刷新失败: {}", e),
            })
        })?;
        Ok(())
    }

    /// 获取已记录的错误数量

    /// 完成记录并显示统计信息
    pub fn finalize(&mut self) -> Result<()> {
        self.flush()?;
        // 写入 summary JSON
        let summary_json = serde_json::to_string_pretty(&self.metrics).map_err(|e| {
            Error::Export(ExportError::SerializationFailed {
                data_type: "ErrorMetrics".to_string(),
                reason: e.to_string(),
            })
        })?;
        std::fs::write(&self.summary_path, summary_json).map_err(|e| {
            Error::Export(ExportError::FileWriteFailed {
                path: self.summary_path.clone(),
                reason: e.to_string(),
            })
        })?;

        if self.count > 0 {
            info!(
                "错误日志已写入: {} ({} 条错误记录, 分类: {:?})",
                self.path, self.count, self.metrics.by_category
            );
            info!("错误指标摘要: {}", self.summary_path);
        } else {
            debug!(
                "无错误记录需要写入 (summary 仍已生成) {}",
                self.summary_path
            );
        }
        Ok(())
    }

    /// 获取 summary 路径（便于测试）
    pub fn summary_path(&self) -> &str {
        &self.summary_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_error_logger_new() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("errors.jsonl");

        let logger = ErrorLogger::new(&log_path)?;
        assert!(log_path.exists());
        assert!(logger.summary_path().ends_with("errors.summary.json"));

        Ok(())
    }

    #[test]
    fn test_error_logger_log_error() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("errors.jsonl");

        let mut logger = ErrorLogger::new(&log_path)?;

        let record = ParseErrorRecord {
            timestamp: "2025-01-09 10:00:00.000".to_string(),
            file_path: "/path/to/file.log".to_string(),
            error_message: "Invalid format".to_string(),
            raw_content: Some("bad line content".to_string()),
            line_number: Some(42),
        };

        logger.log_error(record)?;

        logger.finalize()?;

        // 验证文件内容
        let content = fs::read_to_string(&log_path)?;
        assert!(content.contains("Invalid format"));
        assert!(content.contains("bad line content"));
        assert!(content.contains("\"line_number\":42"));

        Ok(())
    }

    #[test]
    fn test_error_logger_multiple_errors() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("errors.jsonl");

        let mut logger = ErrorLogger::new(&log_path)?;

        for i in 1..=5 {
            let record = ParseErrorRecord {
                timestamp: format!("2025-01-09 10:00:{:02}.000", i),
                file_path: format!("/path/to/file{}.log", i),
                error_message: format!("Error {}", i),
                raw_content: None,
                line_number: Some(i),
            };
            logger.log_error(record)?;
        }

        logger.finalize()?;

        // 验证文件有5行
        let content = fs::read_to_string(&log_path)?;
        assert_eq!(content.lines().count(), 5);

        Ok(())
    }

    #[test]
    fn test_error_logger_creates_parent_directory() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir
            .path()
            .join("logs")
            .join("errors")
            .join("parse.jsonl");

        let mut logger = ErrorLogger::new(&log_path)?;
        assert!(log_path.exists());
        assert!(log_path.parent().unwrap().exists());

        logger.finalize()?;
        Ok(())
    }

    #[test]
    fn test_error_logger_append_mode() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("errors.jsonl");

        // 第一次写入
        {
            let mut logger = ErrorLogger::new(&log_path)?;
            let record = ParseErrorRecord {
                timestamp: "2025-01-09 10:00:00.000".to_string(),
                file_path: "file1.log".to_string(),
                error_message: "Error 1".to_string(),
                raw_content: None,
                line_number: None,
            };
            logger.log_error(record)?;
            logger.finalize()?;

            // 验证 summary 文件
            let summary_path = log_path.parent().unwrap().join("errors.summary.json");
            assert!(summary_path.exists());
            let summary_content = fs::read_to_string(summary_path)?;
            assert!(summary_content.contains("\"total\""));
        }

        // 第二次写入（追加）
        {
            let mut logger = ErrorLogger::new(&log_path)?;
            let record = ParseErrorRecord {
                timestamp: "2025-01-09 10:00:01.000".to_string(),
                file_path: "file2.log".to_string(),
                error_message: "Error 2".to_string(),
                raw_content: None,
                line_number: None,
            };
            logger.log_error(record)?;
            logger.finalize()?;
        }

        // 验证有2行
        let content = fs::read_to_string(&log_path)?;
        assert_eq!(content.lines().count(), 2);
        assert!(content.contains("Error 1"));
        assert!(content.contains("Error 2"));

        Ok(())
    }
}

/// 错误日志记录器 - 将解析失败的原始数据记录到文件
use crate::error::{Error, ExportError, Result};
use log::{debug, info};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// 解析错误记录
#[derive(Debug)]
pub struct ParseErrorRecord {
    /// 错误发生的文件路径
    pub file_path: String,
    /// 错误原因/描述
    pub error_message: String,
    /// 原始数据内容（导致解析失败的行或片段）
    pub raw_content: Option<String>,
    /// 行号（如果适用）
    pub line_number: Option<usize>,
}

/// 错误日志记录器
#[derive(Debug, Default)]
pub struct ErrorMetrics {
    /// 总错误数
    pub total: usize,
    /// 按分类统计
    pub by_category: HashMap<String, usize>,
    /// 解析错误的细分（变体）统计
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

        info!("Error logger initialized: {}", path_str);

        // summary 文件路径处理（使用文本后缀）
        let summary_path = format!("{}.summary.txt", path_str);

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
        // 将记录以可读文本行写入（file | error | raw | line）
        let raw = record.raw_content.clone().unwrap_or_default();
        let line_no = record
            .line_number
            .map(|n| n.to_string())
            .unwrap_or_default();
        let line = format!(
            "{} | {} | {} | {}",
            record.file_path,
            record.error_message,
            raw.replace('\n', "\\n"),
            line_no
        );

        writeln!(self.writer, "{}", line).map_err(|e| {
            Error::Export(ExportError::FileWriteFailed {
                path: PathBuf::from(&self.path),
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
                path: PathBuf::from(&self.path),
                reason: format!("Flush failed: {}", e),
            })
        })?;
        Ok(())
    }

    /// 获取已记录的错误数量

    /// 完成记录并显示统计信息
    pub fn finalize(&mut self) -> Result<()> {
        self.flush()?;
        // 写入 summary 文本（可读格式）
        let mut summary = String::new();
        summary.push_str(&format!("total: {}\n", self.metrics.total));
        for (k, v) in &self.metrics.by_category {
            summary.push_str(&format!("category {}: {}\n", k, v));
        }
        if !self.metrics.parse_variants.is_empty() {
            summary.push_str("parse_variants:\n");
            for (k, v) in &self.metrics.parse_variants {
                summary.push_str(&format!("  {}: {}\n", k, v));
            }
        }

        fs::write(&self.summary_path, summary).map_err(|e| {
            Error::Export(ExportError::FileWriteFailed {
                path: PathBuf::from(&self.summary_path),
                reason: e.to_string(),
            })
        })?;

        if self.count > 0 {
            info!(
                "Error log written: {} ({} records, categories: {:?})",
                self.path, self.count, self.metrics.by_category
            );
            info!("Error summary: {}", self.summary_path);
        } else {
            debug!(
                "No error records to write (summary still generated) {}",
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

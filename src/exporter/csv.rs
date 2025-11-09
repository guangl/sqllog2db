use super::util::{LineBuffer, ensure_parent_dir, open_output_file};
/// CSV 导出器实现
use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// CSV 导出器 - 将 SQL 日志导出为 CSV 格式
pub struct CsvExporter {
    path: PathBuf,
    overwrite: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
    header_written: bool,
    buffer: LineBuffer, // 批量缓冲区抽象
}

impl CsvExporter {
    /// 创建新的 CSV 导出器
    pub fn new(path: impl AsRef<Path>, overwrite: bool) -> Self {
        Self::with_batch_size(path, overwrite, 0)
    }

    /// 创建新的 CSV 导出器（指定批量大小）
    pub fn with_batch_size(path: impl AsRef<Path>, overwrite: bool, batch_size: usize) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            overwrite,
            writer: None,
            stats: ExportStats::new(),
            header_written: false,
            buffer: LineBuffer::new(batch_size),
        }
    }

    /// 从配置创建 CSV 导出器，支持自定义批量大小
    pub fn from_config(config: &crate::config::CsvExporter, batch_size: usize) -> Self {
        if batch_size > 0 {
            Self::with_batch_size(&config.path, config.overwrite, batch_size)
        } else {
            Self::new(&config.path, config.overwrite)
        }
    }

    /// 刷新缓冲区，将所有缓存的行写入文件
    fn flush_buffer(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "CSV 导出器未初始化".to_string(),
            })
        })?;

        let count_before = self.buffer.len();
        let written = self.buffer.flush_all(writer).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("写入失败: {}", e),
            })
        })?;

        debug!("刷新 CSV 缓冲区，写入 {} 条记录", written);
        assert_eq!(written, count_before);
        Ok(())
    }

    /// 写入 CSV 头部
    fn write_header(&mut self) -> Result<()> {
        if self.header_written {
            return Ok(());
        }

        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "CSV 导出器未初始化".to_string(),
            })
        })?;

        // CSV 头部字段
        let header = "timestamp,ep,sess_id,thrd_id,username,trx_id,statement,appname,client_ip,sql,exec_time_ms,row_count,exec_id\n";

        writer.write_all(header.as_bytes()).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("写入 CSV 头部失败: {}", e),
            })
        })?;

        self.header_written = true;
        debug!("CSV 头部已写入");
        Ok(())
    }

    /// 转义 CSV 字段 (处理引号和逗号)
    fn escape_csv_field(field: &str) -> String {
        if field.contains(',') || field.contains('"') || field.contains('\n') {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_string()
        }
    }

    /// 将 Sqllog 转换为 CSV 行
    fn sqllog_to_csv_line(sqllog: &Sqllog) -> String {
        let ts = Self::escape_csv_field(&sqllog.ts);
        let ep = sqllog.meta.ep;
        let sess_id = Self::escape_csv_field(&sqllog.meta.sess_id);
        let thrd_id = Self::escape_csv_field(&sqllog.meta.thrd_id);
        let username = Self::escape_csv_field(&sqllog.meta.username);
        let trxid = Self::escape_csv_field(&sqllog.meta.trxid);
        let statement = Self::escape_csv_field(&sqllog.meta.statement);
        let appname = Self::escape_csv_field(&sqllog.meta.appname);
        let client_ip = Self::escape_csv_field(&sqllog.meta.client_ip);
        let sql = Self::escape_csv_field(&sqllog.body);

        // 可选的性能指标
        let exec_time = sqllog
            .indicators
            .as_ref()
            .map(|i| i.execute_time.to_string())
            .unwrap_or_default();
        let row_count = sqllog
            .indicators
            .as_ref()
            .map(|i| i.row_count.to_string())
            .unwrap_or_default();
        let exec_id = sqllog
            .indicators
            .as_ref()
            .map(|i| i.execute_id.to_string())
            .unwrap_or_default();

        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            ts,
            ep,
            sess_id,
            thrd_id,
            username,
            trxid,
            statement,
            appname,
            client_ip,
            sql,
            exec_time,
            row_count,
            exec_id
        )
    }
}

impl Exporter for CsvExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("初始化 CSV 导出器: {}", self.path.display());

        // 使用 util 模块创建父目录
        ensure_parent_dir(&self.path).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("创建目录失败: {}", e),
            })
        })?;

        // 检查文件是否已存在
        if self.path.exists() && !self.overwrite {
            return Err(Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "文件已存在(使用 overwrite=true 覆盖)".to_string(),
            }));
        }

        // 使用 util 模块打开文件
        let writer = open_output_file(&self.path, self.overwrite).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("打开文件失败: {}", e),
            })
        })?;

        self.writer = Some(writer);

        // 写入头部
        self.write_header()?;

        info!("CSV 导出器初始化成功: {}", self.path.display());
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog) -> Result<()> {
        // 检查是否已初始化
        if self.writer.is_none() {
            return Err(Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "CSV 导出器未初始化".to_string(),
            }));
        }

        let line = Self::sqllog_to_csv_line(sqllog);

        // 根据 batch_size 决定是缓冲还是立即写入
        self.buffer.push(line);
        if self.buffer.should_flush() {
            self.flush_buffer()?;
        }

        // 无论哪种模式，都记录成功（数据已被接受）
        self.stats.record_success();

        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[Sqllog]) -> Result<()> {
        debug!("批量导出 {} 条记录到 CSV", sqllogs.len());

        for sqllog in sqllogs {
            self.export(sqllog)?;
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 刷新剩余的缓冲区数据
        if !self.buffer.is_empty() {
            self.flush_buffer()?;
        }

        if let Some(mut writer) = self.writer.take() {
            writer.flush().map_err(|e| {
                Error::Export(ExportError::CsvExportFailed {
                    path: self.path.clone(),
                    reason: format!("刷新缓冲区失败: {}", e),
                })
            })?;

            info!(
                "CSV 导出完成: {} (成功: {}, 失败: {})",
                self.path.display(),
                self.stats.exported,
                self.stats.failed
            );
        } else {
            warn!("CSV 导出器未初始化或已完成");
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "CSV"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl Drop for CsvExporter {
    fn drop(&mut self) {
        if self.writer.is_some() {
            if let Err(e) = self.finalize() {
                warn!("CSV 导出器 Drop 时完成操作失败: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_database_parser_sqllog::sqllog::{IndicatorsParts, MetaParts};

    fn create_test_sqllog() -> Sqllog {
        Sqllog {
            ts: "2025-01-09 10:00:00.000".to_string(),
            meta: MetaParts {
                ep: 0,
                sess_id: "0x12345".to_string(),
                thrd_id: "456".to_string(),
                username: "test_user".to_string(),
                trxid: "789".to_string(),
                statement: "0x9abc".to_string(),
                appname: "test_app".to_string(),
                client_ip: "127.0.0.1".to_string(),
            },
            body: "[SEL] SELECT * FROM test_table".to_string(),
            indicators: Some(IndicatorsParts {
                execute_time: 10.5,
                row_count: 100,
                execute_id: 12345,
            }),
        }
    }

    #[test]
    fn test_csv_exporter_new() {
        let exporter = CsvExporter::new("test.csv", true);
        assert_eq!(exporter.path, PathBuf::from("test.csv"));
        assert!(exporter.overwrite);
        assert!(exporter.writer.is_none());
        assert_eq!(exporter.stats.exported, 0);
    }

    #[test]
    fn test_escape_csv_field() {
        assert_eq!(CsvExporter::escape_csv_field("simple"), "simple");
        assert_eq!(
            CsvExporter::escape_csv_field("with,comma"),
            "\"with,comma\""
        );
        assert_eq!(
            CsvExporter::escape_csv_field("with\"quote"),
            "\"with\"\"quote\""
        );
        assert_eq!(
            CsvExporter::escape_csv_field("with\nnewline"),
            "\"with\nnewline\""
        );
    }

    #[test]
    fn test_sqllog_to_csv_line() {
        let sqllog = create_test_sqllog();
        let line = CsvExporter::sqllog_to_csv_line(&sqllog);

        assert!(line.contains("2025-01-09 10:00:00.000"));
        assert!(line.contains("test_user"));
        assert!(line.contains("test_app"));
        assert!(line.contains("SELECT * FROM test_table"));
        assert!(line.contains("10.5")); // exec_time
        assert!(line.contains("100")); // row_count
        assert!(line.contains("12345")); // exec_id
    }

    #[test]
    fn test_sqllog_to_csv_line_without_indicators() {
        let sqllog = Sqllog {
            ts: "2025-01-09 10:00:00.000".to_string(),
            meta: MetaParts {
                ep: 0,
                sess_id: "0x12345".to_string(),
                thrd_id: "456".to_string(),
                username: "test_user".to_string(),
                trxid: "789".to_string(),
                statement: "0x9abc".to_string(),
                appname: "test_app".to_string(),
                client_ip: "127.0.0.1".to_string(),
            },
            body: "[SEL] SELECT 1".to_string(),
            indicators: None,
        };

        let line = CsvExporter::sqllog_to_csv_line(&sqllog);

        // 验证必填字段存在
        assert!(line.contains("2025-01-09 10:00:00.000"));
        assert!(line.contains("test_user"));
        assert!(line.contains("SELECT 1"));

        // 验证可选字段为空
        let parts: Vec<&str> = line.trim_end_matches('\n').split(',').collect();
        assert_eq!(parts[10], ""); // exec_time_ms
        assert_eq!(parts[11], ""); // row_count
        assert_eq!(parts[12], ""); // exec_id
    }

    #[test]
    fn test_csv_exporter_from_config() {
        let config = crate::config::CsvExporter {
            path: "output.csv".to_string(),
            overwrite: false,
        };

        let exporter = CsvExporter::from_config(&config, 0);
        assert_eq!(exporter.path, PathBuf::from("output.csv"));
        assert!(!exporter.overwrite);
        assert_eq!(exporter.buffer.len(), 0);
    }

    #[test]
    fn test_csv_exporter_from_config_with_batch() {
        let config = crate::config::CsvExporter {
            path: "output.csv".to_string(),
            overwrite: true,
        };

        let exporter = CsvExporter::from_config(&config, 500);
        assert_eq!(exporter.path, PathBuf::from("output.csv"));
        assert!(exporter.overwrite);
        // LineBuffer 批次大小为 500，这里无法直接读取，验证为初始化成功且无缓冲内容
        assert_eq!(exporter.buffer.len(), 0);
    }

    #[test]
    fn test_csv_exporter_initialize_creates_file() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_csv_init.csv");

        // 确保文件不存在
        let _ = fs::remove_file(&test_file);

        let mut exporter = CsvExporter::new(&test_file, true);
        let result = exporter.initialize();

        assert!(result.is_ok());

        // 必须 finalize 以刷新缓冲区
        exporter.finalize().unwrap();

        assert!(test_file.exists());

        // 验证写入了头部
        let content = fs::read_to_string(&test_file).unwrap();
        assert!(content.contains("timestamp,ep,sess_id"));

        // 清理
        let _ = exporter.finalize();
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_csv_exporter_overwrite_protection() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_csv_overwrite.csv");

        // 创建一个已存在的文件
        fs::write(&test_file, "existing content").unwrap();

        let mut exporter = CsvExporter::new(&test_file, false);
        let result = exporter.initialize();

        assert!(result.is_err());

        // 验证错误类型
        if let Err(e) = result {
            assert!(e.to_string().contains("已存在"));
        }

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_csv_exporter_export_single_record() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_csv_export.csv");

        let _ = fs::remove_file(&test_file);

        let mut exporter = CsvExporter::new(&test_file, true);
        exporter.initialize().unwrap();

        let sqllog = create_test_sqllog();
        let result = exporter.export(&sqllog);

        assert!(result.is_ok());
        assert_eq!(exporter.stats.exported, 1);
        assert_eq!(exporter.stats.failed, 0);

        exporter.finalize().unwrap();

        // 验证文件内容
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2); // 头部 + 1 条记录
        assert!(lines[1].contains("test_user"));

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_csv_exporter_export_batch() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_csv_batch.csv");

        let _ = fs::remove_file(&test_file);

        let mut exporter = CsvExporter::new(&test_file, true);
        exporter.initialize().unwrap();

        // 创建多条记录
        let sqllogs = vec![
            create_test_sqllog(),
            create_test_sqllog(),
            create_test_sqllog(),
        ];

        let result = exporter.export_batch(&sqllogs);

        assert!(result.is_ok());
        assert_eq!(exporter.stats.exported, 3);

        exporter.finalize().unwrap();

        // 验证文件内容
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 4); // 头部 + 3 条记录

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_csv_exporter_export_without_initialize() {
        let mut exporter = CsvExporter::new("test.csv", true);
        let sqllog = create_test_sqllog();

        let result = exporter.export(&sqllog);
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.to_string().contains("未初始化"));
        }
    }

    #[test]
    fn test_csv_exporter_name() {
        let exporter = CsvExporter::new("test.csv", true);
        assert_eq!(exporter.name(), "CSV");
    }

    #[test]
    fn test_csv_exporter_stats() {
        let exporter = CsvExporter::new("test.csv", true);
        let stats = exporter.stats_snapshot().unwrap();
        assert_eq!(stats.exported, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.skipped, 0);
    }

    #[test]
    fn test_csv_exporter_with_batch_size() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_csv_batch.csv");

        let _ = fs::remove_file(&test_file);

        // 创建批量大小为 2 的导出器
        let mut exporter = CsvExporter::with_batch_size(&test_file, true, 2);
        exporter.initialize().unwrap();

        // 导出 5 条记录
        for _ in 0..5 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        assert_eq!(exporter.stats.exported, 5);

        // 应该有 4 条已经写入（2次批量写入），1条在缓冲区
        assert_eq!(exporter.buffer.len(), 1);

        exporter.finalize().unwrap();

        // 验证所有记录都已写入
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 6); // 头部 + 5 条记录

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_csv_exporter_batch_size_zero_buffers_all() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_csv_batch_zero.csv");

        let _ = fs::remove_file(&test_file);

        // batch_size = 0 表示全部缓冲
        let mut exporter = CsvExporter::with_batch_size(&test_file, true, 0);
        exporter.initialize().unwrap();

        // 导出 10 条记录
        for _ in 0..10 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        // 所有记录应该都在缓冲区
        assert_eq!(exporter.buffer.len(), 10);
        assert_eq!(exporter.stats.exported, 10);

        // finalize 时一次性写入
        exporter.finalize().unwrap();

        // 验证所有记录都已写入
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 11); // 头部 + 10 条记录

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_csv_exporter_batch_flush_exact_size() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_csv_batch_exact.csv");

        let _ = fs::remove_file(&test_file);

        // 批量大小为 5
        let mut exporter = CsvExporter::with_batch_size(&test_file, true, 5);
        exporter.initialize().unwrap();

        // 导出正好 5 条记录
        for _ in 0..5 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        // 应该触发一次批量写入，缓冲区清空
        assert_eq!(exporter.buffer.len(), 0);
        assert_eq!(exporter.stats.exported, 5);

        exporter.finalize().unwrap();

        // 验证所有记录都已写入
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 6); // 头部 + 5 条记录

        // 清理
        let _ = fs::remove_file(&test_file);
    }
}

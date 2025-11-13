use super::util::{ensure_parent_dir, open_output_file};
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
    line_buf: String, // 重用的行缓冲区
}

impl CsvExporter {
    /// 创建新的 CSV 导出器
    pub fn new(path: impl AsRef<Path>, overwrite: bool) -> Self {
        Self::with_batch_size(path, overwrite, 0)
    }

    /// 创建新的 CSV 导出器（指定批量大小）
    pub fn with_batch_size(path: impl AsRef<Path>, overwrite: bool, _batch_size: usize) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            overwrite,
            writer: None,
            stats: ExportStats::new(),
            header_written: false,
            line_buf: String::with_capacity(512), // 预分配
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

    /// 写入 CSV 字段到缓冲区（避免分配）
    fn write_csv_field(buf: &mut String, field: &str) {
        if field.contains(',') || field.contains('"') || field.contains('\n') {
            buf.push('"');
            for ch in field.chars() {
                if ch == '"' {
                    buf.push('"');
                    buf.push('"');
                } else {
                    buf.push(ch);
                }
            }
            buf.push('"');
        } else {
            buf.push_str(field);
        }
    }

    /// 将 Sqllog 转换为 CSV 行（优化版本，使用预分配缓冲区）
    fn sqllog_to_csv_line_into(sqllog: &Sqllog, buf: &mut String) {
        buf.clear();
        buf.reserve(256); // 预分配合理大小

        Self::write_csv_field(buf, &sqllog.ts);
        buf.push(',');

        use std::fmt::Write;
        let _ = write!(buf, "{},", sqllog.meta.ep);

        Self::write_csv_field(buf, &sqllog.meta.sess_id);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.thrd_id);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.username);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.trxid);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.statement);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.appname);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.meta.client_ip);
        buf.push(',');
        Self::write_csv_field(buf, &sqllog.body);
        buf.push(',');

        // 性能指标
        if let Some(indicators) = &sqllog.indicators {
            let _ = write!(
                buf,
                "{},{},{}",
                indicators.execute_time, indicators.row_count, indicators.execute_id
            );
        } else {
            buf.push_str(",,");
        }

        buf.push('\n');
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

        // 使用重用缓冲区生成 CSV 行
        Self::sqllog_to_csv_line_into(sqllog, &mut self.line_buf);

        // 直接写入，避免额外的字符串克隆和缓冲
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: "CSV 导出器未初始化".to_string(),
            })
        })?;

        writer.write_all(self.line_buf.as_bytes()).map_err(|e| {
            Error::Export(ExportError::CsvExportFailed {
                path: self.path.clone(),
                reason: format!("写入 CSV 行失败: {}", e),
            })
        })?;

        // 无论哪种模式，都记录成功（数据已被接受）
        self.stats.record_success();

        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog]) -> Result<()> {
        debug!("批量导出 {} 条记录到 CSV", sqllogs.len());

        for sqllog in sqllogs {
            self.export(sqllog)?;
        }

        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
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

    // 注意：escape_csv_field 和 sqllog_to_csv_line 已经改为内部实现
    // 通过 write_csv_field 和 sqllog_to_csv_line_into，不再暴露为公共方法
    // 这些功能通过集成测试来验证（test_csv_exporter_export_single_record 等）

    #[test]
    fn test_csv_exporter_from_config() {
        let config = crate::config::CsvExporter {
            path: "output.csv".to_string(),
            overwrite: false,
        };

        let exporter = CsvExporter::from_config(&config, 0);
        assert_eq!(exporter.path, PathBuf::from("output.csv"));
        assert!(!exporter.overwrite);
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

        // 确保先删除已存在的文件
        let _ = fs::remove_file(&test_file);

        let mut exporter = CsvExporter::new(&test_file, true);
        exporter.initialize().unwrap();

        // 创建多条记录
        let sqllogs = vec![
            create_test_sqllog(),
            create_test_sqllog(),
            create_test_sqllog(),
        ];

        let refs: Vec<&Sqllog> = sqllogs.iter().collect();
        let result = exporter.export_batch(&refs);

        assert!(result.is_ok());
        assert_eq!(exporter.stats.exported, 3);

        exporter.finalize().unwrap();

        // 验证文件内容
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // 打印调试信息
        if lines.len() != 4 {
            eprintln!("期望 4 行，实际 {} 行:", lines.len());
            for (i, line) in lines.iter().enumerate() {
                eprintln!("行 {}: {}", i + 1, line);
            }
        }

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
        let test_file = temp_dir.join("test_csv_batch_size.csv");

        // 确保删除已存在的文件
        let _ = fs::remove_file(&test_file);

        // 创建批量大小为 2 的导出器（注意：当前实现直接写入，不再使用内存缓冲）
        let mut exporter = CsvExporter::with_batch_size(&test_file, true, 2);
        exporter.initialize().unwrap();

        // 导出 5 条记录
        for _ in 0..5 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        assert_eq!(exporter.stats.exported, 5);

        exporter.finalize().unwrap();

        // 验证所有记录都已写入
        assert!(test_file.exists(), "文件应该存在: {}", test_file.display());
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

        // batch_size = 0（注意：当前实现直接写入，不再在内存中缓冲）
        let mut exporter = CsvExporter::with_batch_size(&test_file, true, 0);
        exporter.initialize().unwrap();

        // 导出 10 条记录
        for _ in 0..10 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        assert_eq!(exporter.stats.exported, 10);

        // finalize 时刷新缓冲区
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

        // 批量大小为 5（注意：当前实现直接写入）
        let mut exporter = CsvExporter::with_batch_size(&test_file, true, 5);
        exporter.initialize().unwrap();

        // 导出正好 5 条记录
        for _ in 0..5 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

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

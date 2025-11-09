use super::util::{LineBuffer, ensure_parent_dir, open_output_file};
/// JSONL (JSON Lines) 导出器实现
use super::{ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// JSONL 导出器 - 将 SQL 日志导出为 JSON Lines 格式
/// 每行一个完整的 JSON 对象,便于流式处理
pub struct JsonlExporter {
    path: PathBuf,
    overwrite: bool,
    writer: Option<BufWriter<File>>,
    stats: ExportStats,
    buffer: LineBuffer, // 抽象缓冲区
}

/// 用于序列化的 SQL 日志结构
#[derive(Debug, Serialize)]
struct SqllogRecord {
    timestamp: String,
    ep: u8,
    sess_id: String,
    thrd_id: String,
    username: String,
    trxid: String,
    statement: String,
    appname: String,
    client_ip: String,
    sql: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    exec_time_ms: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    row_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exec_id: Option<i64>,
}

impl From<&Sqllog> for SqllogRecord {
    fn from(sqllog: &Sqllog) -> Self {
        Self {
            timestamp: sqllog.ts.clone(),
            ep: sqllog.meta.ep,
            sess_id: sqllog.meta.sess_id.clone(),
            thrd_id: sqllog.meta.thrd_id.clone(),
            username: sqllog.meta.username.clone(),
            trxid: sqllog.meta.trxid.clone(),
            statement: sqllog.meta.statement.clone(),
            appname: sqllog.meta.appname.clone(),
            client_ip: sqllog.meta.client_ip.clone(),
            sql: sqllog.body.clone(),
            exec_time_ms: sqllog.indicators.as_ref().map(|i| i.execute_time),
            row_count: sqllog.indicators.as_ref().map(|i| i.row_count),
            exec_id: sqllog.indicators.as_ref().map(|i| i.execute_id),
        }
    }
}

impl JsonlExporter {
    /// 创建新的 JSONL 导出器
    pub fn new(path: impl AsRef<Path>, overwrite: bool) -> Self {
        Self::with_batch_size(path, overwrite, 0)
    }

    /// 创建新的 JSONL 导出器（指定批量大小）
    pub fn with_batch_size(path: impl AsRef<Path>, overwrite: bool, batch_size: usize) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            overwrite,
            writer: None,
            stats: ExportStats::new(),
            buffer: LineBuffer::new(batch_size),
        }
    }

    /// 从配置创建 JSONL 导出器，支持自定义批量大小
    pub fn from_config(config: &crate::config::JsonlExporter, batch_size: usize) -> Self {
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
            Error::Export(ExportError::JsonlExportFailed {
                path: self.path.clone(),
                reason: "JSONL 导出器未初始化".to_string(),
            })
        })?;

        let count_before = self.buffer.len();
        let written = self.buffer.flush_all(writer).map_err(|e| {
            Error::Export(ExportError::JsonlExportFailed {
                path: self.path.clone(),
                reason: format!("写入失败: {}", e),
            })
        })?;
        debug!("刷新 JSONL 缓冲区，写入 {} 条记录", written);
        assert_eq!(written, count_before);
        Ok(())
    }
}

impl Exporter for JsonlExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("初始化 JSONL 导出器: {}", self.path.display());

        // 使用 util 模块创建父目录
        ensure_parent_dir(&self.path).map_err(|e| {
            Error::Export(ExportError::JsonlExportFailed {
                path: self.path.clone(),
                reason: format!("创建目录失败: {}", e),
            })
        })?;

        // 检查文件是否已存在
        if self.path.exists() && !self.overwrite {
            return Err(Error::Export(ExportError::JsonlExportFailed {
                path: self.path.clone(),
                reason: "文件已存在(使用 overwrite=true 覆盖)".to_string(),
            }));
        }

        // 使用 util 模块打开文件
        let writer = open_output_file(&self.path, self.overwrite).map_err(|e| {
            Error::Export(ExportError::JsonlExportFailed {
                path: self.path.clone(),
                reason: format!("打开文件失败: {}", e),
            })
        })?;

        self.writer = Some(writer);

        info!("JSONL 导出器初始化成功: {}", self.path.display());
        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog) -> Result<()> {
        // 检查是否已初始化
        if self.writer.is_none() {
            return Err(Error::Export(ExportError::JsonlExportFailed {
                path: self.path.clone(),
                reason: "JSONL 导出器未初始化".to_string(),
            }));
        }

        // 转换为序列化结构
        let record = SqllogRecord::from(sqllog);

        // 序列化为 JSON
        let json = serde_json::to_string(&record).map_err(|e| {
            self.stats.record_failure();
            Error::Export(ExportError::JsonlExportFailed {
                path: self.path.clone(),
                reason: format!("序列化失败: {}", e),
            })
        })?;

        // 根据 batch_size 决定是缓冲还是立即写入
        self.buffer.push(format!("{}\n", json));
        if self.buffer.should_flush() {
            self.flush_buffer()?;
        }

        // 无论哪种模式，都记录成功（数据已被接受）
        self.stats.record_success();

        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[Sqllog]) -> Result<()> {
        debug!("批量导出 {} 条记录到 JSONL", sqllogs.len());

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
                Error::Export(ExportError::JsonlExportFailed {
                    path: self.path.clone(),
                    reason: format!("刷新缓冲区失败: {}", e),
                })
            })?;

            info!(
                "JSONL 导出完成: {} (成功: {}, 失败: {})",
                self.path.display(),
                self.stats.exported,
                self.stats.failed
            );
        } else {
            warn!("JSONL 导出器未初始化或已完成");
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "JSONL"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        Some(self.stats.clone())
    }
}

impl Drop for JsonlExporter {
    fn drop(&mut self) {
        if self.writer.is_some() {
            if let Err(e) = self.finalize() {
                warn!("JSONL 导出器 Drop 时完成操作失败: {}", e);
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
                sess_id: "0x123".to_string(),
                thrd_id: "456".to_string(),
                username: "test_user".to_string(),
                trxid: "789".to_string(),
                statement: "0x999".to_string(),
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
    fn test_jsonl_exporter_new() {
        let exporter = JsonlExporter::new("test.jsonl", true);
        assert_eq!(exporter.path, PathBuf::from("test.jsonl"));
        assert!(exporter.overwrite);
        assert!(exporter.writer.is_none());
        assert_eq!(exporter.stats.exported, 0);
    }

    #[test]
    fn test_sqllog_record_from_sqllog() {
        let sqllog = create_test_sqllog();
        let record = SqllogRecord::from(&sqllog);

        assert_eq!(record.timestamp, "2025-01-09 10:00:00.000");
        assert_eq!(record.ep, 0);
        assert_eq!(record.sess_id, "0x123");
        assert_eq!(record.username, "test_user");
        assert_eq!(record.sql, "[SEL] SELECT * FROM test_table");
        assert_eq!(record.exec_time_ms, Some(10.5));
        assert_eq!(record.row_count, Some(100));
        assert_eq!(record.exec_id, Some(12345));
    }

    #[test]
    fn test_sqllog_record_serialization() {
        let sqllog = create_test_sqllog();
        let record = SqllogRecord::from(&sqllog);

        let json = serde_json::to_string(&record).unwrap();

        assert!(json.contains("\"timestamp\":\"2025-01-09 10:00:00.000\""));
        assert!(json.contains("\"username\":\"test_user\""));
        assert!(json.contains("\"sql\":\"[SEL] SELECT * FROM test_table\""));
        assert!(json.contains("\"exec_time_ms\":10.5"));
        assert!(json.contains("\"row_count\":100"));
    }

    #[test]
    fn test_sqllog_record_without_metrics() {
        let sqllog = Sqllog {
            ts: "2025-01-09 10:00:00.000".to_string(),
            meta: MetaParts {
                ep: 0,
                sess_id: "0x123".to_string(),
                thrd_id: "456".to_string(),
                username: "test_user".to_string(),
                trxid: "789".to_string(),
                statement: "0x999".to_string(),
                appname: "test_app".to_string(),
                client_ip: "127.0.0.1".to_string(),
            },
            body: "[SEL] SELECT 1".to_string(),
            indicators: None,
        };

        let record = SqllogRecord::from(&sqllog);
        let json = serde_json::to_string(&record).unwrap();

        // 可选字段应该被跳过
        assert!(!json.contains("exec_time_ms"));
        assert!(!json.contains("row_count"));
        assert!(!json.contains("exec_id"));
    }

    #[test]
    fn test_jsonl_exporter_from_config() {
        let config = crate::config::JsonlExporter {
            path: "output.jsonl".to_string(),
            overwrite: true,
        };

        let exporter = JsonlExporter::from_config(&config, 0);
        assert_eq!(exporter.path, PathBuf::from("output.jsonl"));
        assert!(exporter.overwrite);
        assert_eq!(exporter.buffer.len(), 0);
    }

    #[test]
    fn test_jsonl_exporter_from_config_with_batch() {
        let config = crate::config::JsonlExporter {
            path: "output.jsonl".to_string(),
            overwrite: false,
        };

        let exporter = JsonlExporter::from_config(&config, 1000);
        assert_eq!(exporter.path, PathBuf::from("output.jsonl"));
        assert!(!exporter.overwrite);
        assert_eq!(exporter.buffer.len(), 0);
    }

    #[test]
    fn test_jsonl_exporter_initialize_creates_file() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_jsonl_init.jsonl");

        // 确保文件不存在
        let _ = fs::remove_file(&test_file);

        let mut exporter = JsonlExporter::new(&test_file, true);
        let result = exporter.initialize();

        assert!(result.is_ok());

        // finalize 后文件应该存在
        exporter.finalize().unwrap();
        assert!(test_file.exists());

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_jsonl_exporter_overwrite_protection() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_jsonl_overwrite.jsonl");

        // 创建一个已存在的文件
        fs::write(&test_file, "existing content").unwrap();

        let mut exporter = JsonlExporter::new(&test_file, false);
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
    fn test_jsonl_exporter_export_single_record() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_jsonl_export.jsonl");

        let _ = fs::remove_file(&test_file);

        let mut exporter = JsonlExporter::new(&test_file, true);
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
        assert_eq!(lines.len(), 1);

        // 验证是有效的 JSON
        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed["username"], "test_user");
        assert_eq!(parsed["sql"], "[SEL] SELECT * FROM test_table");

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_jsonl_exporter_export_batch() {
        use std::fs;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_jsonl_batch.jsonl");

        let _ = fs::remove_file(&test_file);

        let mut exporter = JsonlExporter::new(&test_file, true);
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
        assert_eq!(lines.len(), 3);

        // 验证每一行都是有效的 JSON
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(parsed["username"], "test_user");
        }

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_jsonl_exporter_export_without_initialize() {
        let mut exporter = JsonlExporter::new("test.jsonl", true);
        let sqllog = create_test_sqllog();

        let result = exporter.export(&sqllog);
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.to_string().contains("未初始化"));
        }
    }

    #[test]
    fn test_jsonl_exporter_name() {
        let exporter = JsonlExporter::new("test.jsonl", true);
        assert_eq!(exporter.name(), "JSONL");
    }

    #[test]
    fn test_jsonl_exporter_stats() {
        let exporter = JsonlExporter::new("test.jsonl", true);
        let stats = exporter.stats_snapshot().unwrap();
        assert_eq!(stats.exported, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.skipped, 0);
    }

    #[test]
    fn test_jsonl_exporter_with_batch_size() {
        use std::fs;
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_jsonl_batch.jsonl");

        // 创建批量大小为 3 的导出器
        let mut exporter = JsonlExporter::with_batch_size(&test_file, true, 3);
        exporter.initialize().unwrap();

        // 导出 7 条记录
        for _ in 0..7 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        assert_eq!(exporter.stats.exported, 7);

        // 应该有 6 条已经写入（2次批量写入：3+3），1条在缓冲区
        assert_eq!(exporter.buffer.len(), 1);

        exporter.finalize().unwrap();

        // 验证所有记录都已写入
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 7); // 7 条 JSON 记录

        // 验证每行都是有效的 JSON
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(parsed["username"], "test_user");
        }

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_jsonl_exporter_batch_size_zero_buffers_all() {
        use std::fs;
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_jsonl_batch_zero.jsonl");

        // batch_size = 0 表示全部缓冲
        let mut exporter = JsonlExporter::with_batch_size(&test_file, true, 0);
        exporter.initialize().unwrap();

        // 导出 15 条记录
        for _ in 0..15 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        // 所有记录应该都在缓冲区
        assert_eq!(exporter.buffer.len(), 15);
        assert_eq!(exporter.stats.exported, 15);

        // finalize 时一次性写入
        exporter.finalize().unwrap();

        // 验证所有记录都已写入
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 15);

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_jsonl_exporter_batch_flush_exact_size() {
        use std::fs;
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_jsonl_batch_exact.jsonl");

        // 批量大小为 4
        let mut exporter = JsonlExporter::with_batch_size(&test_file, true, 4);
        exporter.initialize().unwrap();

        // 导出正好 4 条记录
        for _ in 0..4 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        // 应该触发一次批量写入，缓冲区清空
        assert_eq!(exporter.buffer.len(), 0);
        assert_eq!(exporter.stats.exported, 4);

        exporter.finalize().unwrap();

        // 验证所有记录都已写入
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 4);

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_jsonl_exporter_mixed_batch_operations() {
        use std::fs;
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_jsonl_batch_mixed.jsonl");

        // 批量大小为 5
        let mut exporter = JsonlExporter::with_batch_size(&test_file, true, 5);
        exporter.initialize().unwrap();

        // 先导出 3 条
        for _ in 0..3 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        assert_eq!(exporter.buffer.len(), 3); // 未达到批量大小

        // 再导出 5 条，会触发一次刷新
        for _ in 0..5 {
            let sqllog = create_test_sqllog();
            exporter.export(&sqllog).unwrap();
        }

        // 3 + 5 = 8，第一次刷新了 5 条，剩余 3 条
        assert_eq!(exporter.buffer.len(), 3);
        assert_eq!(exporter.stats.exported, 8);

        exporter.finalize().unwrap();

        // 验证所有 8 条记录都已写入
        let content = fs::read_to_string(&test_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 8);

        // 清理
        let _ = fs::remove_file(&test_file);
    }
}

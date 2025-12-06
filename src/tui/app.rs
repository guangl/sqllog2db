#[cfg(feature = "tui")]
use std::sync::{Arc, Mutex};

/// TUI 应用状态
#[cfg(feature = "tui")]
#[derive(Debug, Clone)]
pub struct TuiApp {
    /// 当前处理文件索引
    pub current_file_index: usize,
    /// 总文件数
    pub total_files: usize,
    /// 当前文件名
    pub current_file_name: String,
    /// 已导出记录数
    pub exported_records: usize,
    /// 错误记录数
    pub error_records: usize,
    /// 开始时间（秒）
    pub start_time_secs: u64,
    /// 是否完成
    pub is_finished: bool,
    /// 导出器名称
    pub exporter_name: String,
}

#[cfg(feature = "tui")]
impl TuiApp {
    pub fn new(total_files: usize, exporter_name: String) -> Self {
        Self {
            current_file_index: 0,
            total_files,
            current_file_name: String::new(),
            exported_records: 0,
            error_records: 0,
            start_time_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            is_finished: false,
            exporter_name,
        }
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.current_file_index as f64 / self.total_files as f64) * 100.0
        }
    }

    pub fn elapsed_secs(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
            .saturating_sub(self.start_time_secs)
    }

    pub fn throughput(&self) -> f64 {
        let elapsed = self.elapsed_secs() as f64;
        if elapsed > 0.0 && self.exported_records > 0 {
            self.exported_records as f64 / elapsed
        } else {
            0.0
        }
    }
}

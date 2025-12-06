#[cfg(feature = "tui")]
use std::sync::Arc;
#[cfg(feature = "tui")]
use super::progress::ProgressTracker;
#[cfg(feature = "tui")]
use std::time::Instant;

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
    /// 任务开始时间
    pub start_time: Option<Instant>,
    /// 是否完成
    pub is_finished: bool,
    /// 导出器名称
    pub exporter_name: String,
    /// 进度跟踪器（可选，用于同步共享状态）
    #[cfg_attr(feature = "tui", allow(dead_code))]
    progress_tracker: Option<ProgressTracker>,
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
            start_time: None,
            is_finished: false,
            exporter_name,
            progress_tracker: None,
        }
    }

    pub fn with_progress_tracker(mut self, tracker: ProgressTracker) -> Self {
        self.progress_tracker = Some(tracker);
        self
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    pub fn set_file(&mut self, index: usize, name: String) {
        self.current_file_index = index;
        self.current_file_name = name;
    }

    pub fn add_records(&mut self, count: usize) {
        self.exported_records += count;
    }

    pub fn add_errors(&mut self, count: usize) {
        self.error_records += count;
    }

    pub fn finish(&mut self) {
        self.is_finished = true;
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.current_file_index as f64 / self.total_files as f64) * 100.0
        }
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    pub fn throughput(&self) -> f64 {
        let elapsed = self.elapsed_secs();
        if elapsed > 0.0 && self.exported_records > 0 {
            self.exported_records as f64 / elapsed
        } else {
            0.0
        }
    }
}

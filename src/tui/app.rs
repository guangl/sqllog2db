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
    #[must_use]
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

    #[must_use]
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

    #[must_use]
    pub fn progress_percent(&self) -> u16 {
        if self.total_files == 0 {
            return 0;
        }

        let scaled = self.current_file_index.saturating_mul(100) / self.total_files;
        u16::try_from(scaled.min(100)).unwrap_or(100)
    }

    #[must_use]
    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.map_or(0.0, |t| t.elapsed().as_secs_f64())
    }

    #[must_use]
    pub fn throughput(&self) -> u64 {
        let elapsed_ms = self.start_time.map_or(0, |t| t.elapsed().as_millis());

        if elapsed_ms > 0 && self.exported_records > 0 {
            let per_sec = (self.exported_records as u128 * 1_000) / elapsed_ms;
            u64::try_from(per_sec).unwrap_or(u64::MAX)
        } else {
            0
        }
    }
}

/// TUI 进度事件系统
/// 用于导出任务将进度信息通过通道发送给 TUI
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// 进度事件
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// 任务开始
    Started {
        total_files: usize,
        exporter_name: String,
    },
    /// 文件开始处理
    FileStarted {
        file_index: usize,
        file_name: String,
    },
    /// 批次导出完成
    BatchExported {
        file_index: usize,
        records: usize,
        errors: usize,
    },
    /// 文件处理完成
    FileCompleted { file_index: usize },
    /// 所有文件处理完成
    Completed {
        total_records: usize,
        total_errors: usize,
        elapsed_secs: f64,
    },
    /// 错误发生
    Error { message: String },
}

/// 共享的进度跟踪器
/// 用于在导出线程中原子地更新统计信息
#[derive(Debug, Clone)]
pub struct ProgressTracker {
    current_file_index: Arc<AtomicU64>,
    total_records: Arc<AtomicU64>,
    total_errors: Arc<AtomicU64>,
}

impl ProgressTracker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_file_index: Arc::new(AtomicU64::new(0)),
            total_records: Arc::new(AtomicU64::new(0)),
            total_errors: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set_file_index(&self, index: u64) {
        self.current_file_index.store(index, Ordering::Relaxed);
    }

    pub fn add_records(&self, count: u64) {
        self.total_records.fetch_add(count, Ordering::Relaxed);
    }

    pub fn add_errors(&self, count: u64) {
        self.total_errors.fetch_add(count, Ordering::Relaxed);
    }

    #[must_use]
    pub fn get_file_index(&self) -> u64 {
        self.current_file_index.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn get_total_records(&self) -> u64 {
        self.total_records.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn get_total_errors(&self) -> u64 {
        self.total_errors.load(Ordering::Relaxed)
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

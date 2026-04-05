/// 断点续传状态管理
///
/// 每个文件处理完成后，将其路径 + size + mtime 指纹写入状态文件。
/// 下次携带 `--resume` 运行时，指纹匹配的文件直接跳过。
use crate::error::{Error, FileError, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ResumeState {
    #[serde(default)]
    pub processed: Vec<ProcessedFile>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessedFile {
    pub path: String,
    /// 文件字节数
    pub size: u64,
    /// Unix 时间戳（秒），取自文件 mtime
    pub mtime: u64,
    /// 本次导出的记录数
    pub records: u64,
    /// 处理完成时间（ISO 8601）
    pub processed_at: String,
}

impl ResumeState {
    /// 从状态文件加载；文件不存在时返回空状态。
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        toml::from_str(&content).unwrap_or_default()
    }

    /// 持久化到状态文件。
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).map_err(|e| {
            Error::File(FileError::WriteFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })?;
        if let Some(parent) = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty() && !p.exists())
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::File(FileError::CreateDirectoryFailed {
                    path: parent.to_path_buf(),
                    reason: e.to_string(),
                })
            })?;
        }
        std::fs::write(path, content).map_err(|e| {
            Error::File(FileError::WriteFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })
    }

    /// 判断某文件是否已处理（path + size + mtime 全部匹配）。
    #[must_use]
    pub fn is_processed(&self, file_path: &Path) -> bool {
        let Ok(meta) = std::fs::metadata(file_path) else {
            return false;
        };
        let size = meta.len();
        let mtime = mtime_secs(&meta);
        let path_str = file_path.to_string_lossy();
        self.processed
            .iter()
            .any(|p| p.path == path_str && p.size == size && p.mtime == mtime)
    }

    /// 将文件标记为已处理，并更新已有条目（若存在）。
    pub fn mark_processed(&mut self, file_path: &Path, records: u64) -> Result<()> {
        let meta = std::fs::metadata(file_path).map_err(|e| {
            Error::File(FileError::WriteFailed {
                path: file_path.to_path_buf(),
                reason: e.to_string(),
            })
        })?;
        let path_str = file_path.to_string_lossy().into_owned();
        self.processed.retain(|p| p.path != path_str);
        self.processed.push(ProcessedFile {
            path: path_str,
            size: meta.len(),
            mtime: mtime_secs(&meta),
            records,
            processed_at: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        });
        Ok(())
    }

    /// 返回已处理文件数量。
    #[must_use]
    pub fn processed_count(&self) -> usize {
        self.processed.len()
    }
}

fn mtime_secs(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_nonexistent_returns_empty() {
        let state = ResumeState::load(Path::new("/nonexistent/state.toml"));
        assert!(state.processed.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let state_path = dir.path().join("state.toml");

        let log_file = dir.path().join("test.log");
        std::fs::write(&log_file, "data").unwrap();

        let mut state = ResumeState::default();
        state.mark_processed(&log_file, 42).unwrap();
        state.save(&state_path).unwrap();

        let loaded = ResumeState::load(&state_path);
        assert_eq!(loaded.processed.len(), 1);
        assert_eq!(loaded.processed[0].records, 42);
    }

    #[test]
    fn test_is_processed_matches_fingerprint() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_file = dir.path().join("a.log");
        std::fs::write(&log_file, "hello").unwrap();

        let mut state = ResumeState::default();
        assert!(!state.is_processed(&log_file));

        state.mark_processed(&log_file, 1).unwrap();
        assert!(state.is_processed(&log_file));
    }

    #[test]
    fn test_is_processed_false_after_file_changes() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_file = dir.path().join("a.log");
        std::fs::write(&log_file, "hello").unwrap();

        let mut state = ResumeState::default();
        state.mark_processed(&log_file, 1).unwrap();

        // Overwrite with different content → size changes → fingerprint mismatch
        std::fs::write(&log_file, "hello world extended").unwrap();
        assert!(!state.is_processed(&log_file));
    }

    #[test]
    fn test_mark_processed_updates_existing_entry() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_file = dir.path().join("a.log");
        std::fs::write(&log_file, "hello").unwrap();

        let mut state = ResumeState::default();
        state.mark_processed(&log_file, 10).unwrap();
        state.mark_processed(&log_file, 20).unwrap();

        // Should only have one entry
        assert_eq!(state.processed.len(), 1);
        assert_eq!(state.processed[0].records, 20);
    }

    #[test]
    fn test_processed_count() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut state = ResumeState::default();

        for i in 0..3u8 {
            let f = dir.path().join(format!("{i}.log"));
            std::fs::write(&f, [i]).unwrap();
            state.mark_processed(&f, u64::from(i)).unwrap();
        }
        assert_eq!(state.processed_count(), 3);
    }

    #[test]
    fn test_save_creates_parent_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        let state_path = dir.path().join("subdir").join("state.toml");
        let state = ResumeState::default();
        state.save(&state_path).unwrap();
        assert!(state_path.exists());
    }
}

#[cfg(any(feature = "csv", feature = "parquet", feature = "jsonl"))]
use std::{fs, io, path::Path};

/// 确保输出文件的父目录存在
#[cfg(any(feature = "csv", feature = "parquet", feature = "jsonl"))]
pub fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.exists()) {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

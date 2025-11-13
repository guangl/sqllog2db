use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use std::path::Path;

/// 确保输出文件的父目录存在
pub fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// 根据 overwrite 策略创建/打开输出文件，返回 BufWriter
pub fn open_output_file(path: &Path, overwrite: bool) -> std::io::Result<BufWriter<File>> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(overwrite)
        .open(path)?;
    // 使用 8MB 缓冲区以减少系统调用，提升 NVMe SSD 性能
    Ok(BufWriter::with_capacity(8 * 1024 * 1024, file))
}

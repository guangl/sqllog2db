use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
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

/// 通用的按行缓冲器，负责控制批量大小与统一写出
#[derive(Default)]
pub struct LineBuffer {
    lines: Vec<String>,
    batch_size: usize,
}

impl LineBuffer {
    pub fn new(batch_size: usize) -> Self {
        Self {
            lines: Vec::new(),
            batch_size,
        }
    }

    /// 追加一行（包含换行符或不包含由调用方决定）
    pub fn push(&mut self, line: String) {
        self.lines.push(line);
    }

    /// 缓冲区长度
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// 是否达到批量阈值
    pub fn should_flush(&self) -> bool {
        self.batch_size > 0 && self.lines.len() >= self.batch_size
    }

    /// 将所有缓冲的行写入给定 writer，并清空缓冲
    pub fn flush_all<W: Write>(&mut self, writer: &mut W) -> std::io::Result<usize> {
        let n = self.lines.len();
        for line in &self.lines {
            writer.write_all(line.as_bytes())?;
        }
        self.lines.clear();
        Ok(n)
    }
}

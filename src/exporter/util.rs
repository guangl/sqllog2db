#[cfg(any(feature = "csv", feature = "parquet", feature = "jsonl"))]
use std::{fs, io, path::Path};

/// Saturating cast from f32 milliseconds to i64 milliseconds without precision-loss warnings
#[cfg(any(feature = "csv", feature = "parquet"))]
#[must_use]
pub fn f32_ms_to_i64(ms: f32) -> i64 {
    if !ms.is_finite() {
        return 0;
    }

    const MAX_I64_F64: f64 = 9_223_372_036_854_775_807.0; // i64::MAX as f64
    const MIN_I64_F64: f64 = -9_223_372_036_854_775_808.0; // i64::MIN as f64

    let ms_f64 = f64::from(ms);
    if ms_f64 > MAX_I64_F64 {
        i64::MAX
    } else if ms_f64 < MIN_I64_F64 {
        i64::MIN
    } else {
        let clamped = ms_f64.trunc();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "value already clamped to i64 range"
        )]
        {
            clamped as i64
        }
    }
}

/// 确保输出文件的父目录存在
#[cfg(any(feature = "csv", feature = "parquet", feature = "jsonl"))]
pub fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.exists()) {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

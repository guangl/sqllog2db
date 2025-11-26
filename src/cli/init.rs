use crate::error::Result;
use log::{debug, error, info, warn};
use std::fs;
use std::path::Path;

/// 生成默认配置文件
pub fn handle_init(output_path: &str, force: bool) -> Result<()> {
    let path = Path::new(output_path);

    info!("Preparing to generate configuration file: {}", output_path);

    // 检查文件是否已存在
    if path.exists() && !force {
        error!("Configuration file already exists: {}", output_path);
        info!("Tip: use --force to overwrite");
        return Err(crate::error::Error::File(
            crate::error::FileError::AlreadyExists {
                path: path.to_path_buf(),
            },
        ));
    }

    if path.exists() && force {
        warn!("Will overwrite existing configuration file");
    }

    // 生成默认配置内容
    debug!("Generating default configuration content...");
    let default_config = r#"# SQL 日志导出工具默认配置文件 (请根据需要修改)

[sqllog]
# SQL 日志目录或文件路径
directory = "sqllogs"
# 批量提交大小 (推荐 10000 以获得最佳性能)
# 0 表示全部解析完成后一次性写入; >0 表示每 N 条记录批量写入
batch_size = 10000

[error]
# 解析错误日志(JSON Lines 格式)输出路径
file = "errors.jsonl"

[logging]
# 应用日志输出文件路径 (如 logs/sqllog2db.log)
file = "logs/sqllog2db.log"
# 日志级别: trace | debug | info | warn | error
level = "info"
# 日志保留天数 (1-365) - 用于滚动文件最大保留数量
retention_days = 7

# ===================== 导出器配置 =====================
# 只能配置一个导出器 (CSV / Parquet 二选一)
# 同时配置多个时, 按优先级使用: CSV > Parquet

# 方案 1: CSV 导出(默认)
[exporter.csv]
file = "export/sqllog2db.csv"
overwrite = true
append = false

# 方案 2: Parquet 导出(使用时注释掉上面的导出器, 启用下面的 Parquet)
[exporter.parquet]
file = "export/sqllog2db.parquet"
overwrite = true
row_group_size = 100000 # rows per row group
use_dictionary = true   # enable dictionary encoding

"#;

    // 创建目录（如果需要）
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            info!("Creating directory: {}", parent.display());
            fs::create_dir_all(parent).map_err(|e| {
                crate::error::Error::File(crate::error::FileError::CreateDirectoryFailed {
                    path: parent.to_path_buf(),
                    reason: e.to_string(),
                })
            })?;
        }
    }

    // 写入配置文件
    debug!("Writing configuration file...");
    fs::write(path, default_config).map_err(|e| {
        crate::error::Error::File(crate::error::FileError::WriteFailed {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })
    })?;

    if force && path.exists() {
        info!("Configuration file overwritten: {}", output_path);
    } else {
        info!("Configuration file generated: {}", output_path);
    }

    info!("Next steps:");
    info!("  1. Edit configuration file: {}", output_path);
    info!(
        "  2. Validate configuration: sqllog2db validate -c {}",
        output_path
    );
    info!("  3. Run export: sqllog2db run -c {}", output_path);

    Ok(())
}

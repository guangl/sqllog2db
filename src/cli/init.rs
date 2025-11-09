use tracing::{debug, error, info, warn};

use crate::error::Result;
use std::fs;
use std::path::Path;

/// 生成默认配置文件
pub fn handle_init(output_path: &str, force: bool) -> Result<()> {
    let path = Path::new(output_path);

    info!("准备生成配置文件: {}", output_path);

    // 检查文件是否已存在
    if path.exists() && !force {
        error!("配置文件已存在: {}", output_path);
        info!("提示: 使用 --force 参数强制覆盖");
        return Err(crate::error::Error::File(
            crate::error::FileError::AlreadyExists {
                path: path.to_path_buf(),
            },
        ));
    }

    if path.exists() && force {
        warn!("将覆盖已存在的配置文件");
    }

    // 生成默认配置内容
    debug!("生成默认配置内容...");
    let default_config = r#"# SQL 日志导出工具配置文件

[sqllog]
# SQL 日志文件路径或目录
path = "sqllog"
# 处理线程数 (0 表示自动根据文件数量以及 cpu 核心数量决定)
thread_count = 0

[error]
# 错误日志输出路径
path = "errors.jsonl"

[logging]
# 应用日志输出路径
path = "logs/sqllog2db.log"
# 日志级别: trace, debug, info, warn, error
level = "info"
# 日志保留天数 (1-365)
retention_days = 7

[features]
# 是否替换 SQL 参数（将 ? 替换为实际值）
replace_sql_parameters = false
# 是否分散导出（按日期或其他维度分散到多个文件）
scatter = false

# CSV 导出配置（可配置多个）
[[exporter.csv]]
path = "export/sqllog2db.csv"
overwrite = true

# JSONL 导出配置（可配置多个）
# [[exporter.jsonl]]
# path = "export/sqllog2db.jsonl"
# overwrite = true

# 数据库导出配置（可配置多个）
# [[exporter.database]]
# host = "localhost"
# port = 5236
# username = "admin"
# password = "password"
# overwrite = true
# table_name = "sqllog"
"#;

    // 创建目录（如果需要）
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            info!("创建目录: {}", parent.display());
            fs::create_dir_all(parent).map_err(|e| {
                crate::error::Error::File(crate::error::FileError::CreateDirectoryFailed {
                    path: parent.to_path_buf(),
                    reason: e.to_string(),
                })
            })?;
        }
    }

    // 写入配置文件
    debug!("写入配置文件...");
    fs::write(path, default_config).map_err(|e| {
        crate::error::Error::File(crate::error::FileError::WriteFailed {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })
    })?;

    if force && path.exists() {
        info!("配置文件已覆盖: {}", output_path);
    } else {
        info!("配置文件已生成: {}", output_path);
    }

    info!("下一步:");
    info!("  1. 编辑配置文件: {}", output_path);
    info!("  2. 验证配置: sqllog2db validate -c {}", output_path);
    info!("  3. 运行导出: sqllog2db run -c {}", output_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_init_creates_file() {
        let test_path = "test_init_config.toml";

        // 清理可能存在的测试文件
        let _ = fs::remove_file(test_path);

        // 生成配置
        let result = handle_init(test_path, false);
        assert!(result.is_ok());

        // 验证文件存在
        assert!(Path::new(test_path).exists());

        // 读取并验证内容包含关键字段
        let content = fs::read_to_string(test_path).unwrap();
        assert!(content.contains("[sqllog]"));
        assert!(content.contains("[error]"));
        assert!(content.contains("[logging]"));
        assert!(content.contains("[features]"));
        assert!(content.contains("[[exporter.csv]]"));

        // 清理
        fs::remove_file(test_path).unwrap();
    }

    #[test]
    fn test_init_fails_if_exists_without_force() {
        let test_path = "test_existing_config.toml";

        // 创建一个已存在的文件
        fs::write(test_path, "existing content").unwrap();

        // 尝试生成（不使用 force）
        let result = handle_init(test_path, false);
        assert!(result.is_err());

        // 清理
        fs::remove_file(test_path).unwrap();
    }

    #[test]
    fn test_init_overwrites_with_force() {
        let test_path = "test_force_config.toml";

        // 创建一个已存在的文件
        fs::write(test_path, "old content").unwrap();

        // 使用 force 覆盖
        let result = handle_init(test_path, true);
        assert!(result.is_ok());

        // 验证内容已更新
        let content = fs::read_to_string(test_path).unwrap();
        assert!(content.contains("[sqllog]"));
        assert!(!content.contains("old content"));

        // 清理
        fs::remove_file(test_path).unwrap();
    }
}

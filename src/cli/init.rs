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
    let default_config = r#"# SQL 日志导出工具默认配置文件 (请根据需要修改)

[sqllog]
# SQL 日志目录或文件路径
path = "sqllogs"
# 处理线程数 (0 表示自动，根据文件数量与 CPU 核心数决定)
thread_count = 0
# 批量提交大小 (0 表示全部解析完成后一次性写入; >0 表示每 N 条记录批量写入)
batch_size = 0

[error]
# 解析错误日志（JSON Lines 格式）输出路径
path = "errors.jsonl"

[logging]
# 应用日志输出目录或文件路径 (当前版本要求为“文件路径”，例如 logs/sqllog2db.log)
# 如果仅设置为目录（如 "logs"），请确保后续代码逻辑能够自动生成文件；否则请填写完整文件路径
path = "logs/sqllog2db.log"
# 日志级别: trace | debug | info | warn | error
level = "info"
# 日志保留天数 (1-365) - 用于滚动文件最大保留数量
retention_days = 7

[features]
# 是否替换 SQL 中的参数占位符（如 ? -> 实际值）
replace_sql_parameters = false
# 是否启用分散导出（按日期或其他维度拆分输出文件）
scatter = false

# ===================== 导出器配置 =====================
# 至少需要配置一个导出器 (CSV / JSONL / Database)

# CSV 导出（可配置多个）
[[exporter.csv]]
path = "export/sqllog2db.csv"
overwrite = true

# JSONL 导出（可配置多个）
[[exporter.jsonl]]
path = "export/sqllog2db.jsonl"
overwrite = true

# 数据库导出（可配置多个）示例：文件型数据库 (SQLite / DuckDB)
# [[exporter.database]]
# database_type = "sqlite" # 可选: sqlite | duckdb | postgres | oracle | dm
# path = "export/sqllog2db.sqlite" # 文件型数据库使用 path
# overwrite = true
# table_name = "sqllog"
# batch_size = 1000

# 网络型数据库示例 (DM/PostgreSQL/Oracle)
# [[exporter.database]]
# database_type = "dm"
# host = "localhost"
# port = 5236
# username = "SYSDBA"
# password = "SYSDBA"
# overwrite = true
# table_name = "sqllog"
# batch_size = 1000
# database = "TEST"          # 可选 (postgres/dm)
# service_name = "ORCL"       # Oracle 可选（与 sid 二选一）
# sid = "ORCLSID"             # Oracle 可选（与 service_name 二选一）
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
        assert!(content.contains("retention_days"));
        assert!(content.contains("batch_size"));
        assert!(content.contains("[features]"));
        assert!(content.contains("[[exporter.csv]]"));
        assert!(content.contains("[[exporter.jsonl]]"));

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
        assert!(content.contains("batch_size"));
        assert!(!content.contains("old content"));

        // 清理
        fs::remove_file(test_path).unwrap();
    }
}

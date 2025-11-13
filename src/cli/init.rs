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
# 批量提交大小 (推荐 10000 以获得最佳性能)
# 0 表示全部解析完成后一次性写入; >0 表示每 N 条记录批量写入
batch_size = 10000

[error]
# 解析错误日志（JSON 格式）输出路径
path = "errors.json"

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
# 只能配置一个导出器 (CSV / Database 三选一)
# 同时配置多个时，按优先级使用：CSV > Database

# 方案 1: CSV 导出（默认）
[exporter.csv]
path = "export/sqllog2db.csv"
overwrite = true

# 方案 2: 数据库导出（使用时注释掉上面的导出器，启用下面的 Database）
# 文件型数据库示例 (SQLite)
# [exporter.database]
# database_type = "sqlite" # 可选: sqlite | dm
# path = "export/sqllog2db.sqlite" # 文件型数据库使用 path
# overwrite = true
# table_name = "sqllog"
# batch_size = 1000

# 网络型数据库示例 (DM/PostgreSQL/Oracle)
# [exporter.database]
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

use super::{CsvExporter, ExportStats, Exporter};
use crate::error::{Error, ExportError, Result};
use dm_database_parser_sqllog::Sqllog;
use log::info;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct DmExporter {
    userid: String,
    table_name: String,
    control_file: String,
    data_file: String, // 临时 CSV 文件路径，自动生成
    log_dir: String,
    csv_exporter: Option<CsvExporter>,
}

impl std::fmt::Debug for DmExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DmExporter")
            .field("userid", &self.userid)
            .field("table_name", &self.table_name)
            .field("control_file", &self.control_file)
            .field("log_dir", &self.log_dir)
            .finish()
    }
}

impl DmExporter {
    pub fn from_config(config: &crate::config::DmExporter) -> Self {
        // 从 control_file 路径生成临时 CSV 文件路径
        let data_file = if let Some(parent) = Path::new(&config.control_file).parent() {
            parent.join("sqllog_temp.csv").display().to_string()
        } else {
            "sqllog_temp.csv".to_string()
        };

        Self {
            userid: config.userid.to_string(),
            table_name: config.table_name.to_string(),
            control_file: config.control_file.to_string(),
            data_file,
            log_dir: config.log_dir.to_string(),
            csv_exporter: None,
        }
    }

    fn generate_control_file(&self) -> Result<()> {
        // 获取绝对路径并转换为正常格式（去除 Windows 的 \\?\ 前缀）
        let data_file_abs = std::fs::canonicalize(&self.data_file)
            .map_err(|e| {
                Error::Export(ExportError::IoError {
                    path: self.data_file.clone().into(),
                    reason: format!("Failed to get absolute path: {}", e),
                })
            })?
            .display()
            .to_string()
            .replace(r"\\?\", "")
            .replace("\\", "/");

        let content = format!(
            r#"LOAD DATA
INFILE '{}'
INTO TABLE {}
FIELDS ','
(
    ts,
    ep,
    sess_id,
    thrd_id,
    username,
    trx_id,
    statement,
    appname,
    client_ip,
    sql_text,
    exec_time_ms,
    row_count,
    exec_id
)"#,
            data_file_abs, self.table_name
        );

        let mut file = File::create(&self.control_file).map_err(|e| {
            Error::Export(ExportError::IoError {
                path: self.control_file.clone().into(),
                reason: e.to_string(),
            })
        })?;

        write!(file, "{}", content).map_err(|e| {
            Error::Export(ExportError::IoError {
                path: self.control_file.clone().into(),
                reason: e.to_string(),
            })
        })?;

        Ok(())
    }

    fn create_table_if_not_exists(&self) -> Result<()> {
        info!("Creating table if not exists...");

        // 创建建表 SQL
        let create_table_sql = format!(
            r#"
CREATE TABLE IF NOT EXISTS {} (
    id BIGINT IDENTITY(1,1) PRIMARY KEY,
    ts VARCHAR(64) NOT NULL,
    ep INT NOT NULL,
    sess_id VARCHAR(128) NOT NULL,
    thrd_id VARCHAR(128) NOT NULL,
    username VARCHAR(128) NOT NULL,
    trx_id VARCHAR(128) NOT NULL,
    statement VARCHAR(128) NOT NULL,
    appname VARCHAR(256) NOT NULL,
    client_ip VARCHAR(64) NOT NULL,
    sql_text CLOB NOT NULL,
    exec_time_ms FLOAT,
    row_count BIGINT,
    exec_id BIGINT
);
"#,
            self.table_name
        );

        // 写入临时 SQL 文件
        let sql_file = Path::new(&self.log_dir).join("create_table.sql");
        let mut file = File::create(&sql_file).map_err(|e| {
            Error::Export(ExportError::IoError {
                path: sql_file.clone(),
                reason: e.to_string(),
            })
        })?;

        write!(file, "{}", create_table_sql).map_err(|e| {
            Error::Export(ExportError::IoError {
                path: sql_file.clone(),
                reason: e.to_string(),
            })
        })?;

        drop(file);

        // 使用 disql 执行建表 SQL
        let output = Command::new("disql")
            .arg(&self.userid)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        match output {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(create_table_sql.as_bytes());
                    let _ = stdin.write_all(b"\nEXIT;\n");
                }

                let output = child.wait_with_output().map_err(|e| {
                    Error::Export(ExportError::ExternalToolError {
                        tool: "disql".to_string(),
                        reason: format!("Failed to wait for disql: {}", e),
                    })
                })?;

                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if !output.status.success() {
                    info!("Table creation output: {}", stdout);
                    info!("Table creation stderr: {}", stderr);
                    // 不返回错误，因为表可能已存在
                }

                info!("Table creation completed");
                Ok(())
            }
            Err(e) => {
                // disql 不可用时给出警告，但不中断流程
                info!(
                    "Warning: disql not available, please ensure table exists: {}",
                    e
                );
                Ok(())
            }
        }
    }
}

impl Exporter for DmExporter {
    fn initialize(&mut self) -> Result<()> {
        info!("Initializing DM exporter...");

        // 初始化 CSV 导出器（CSV 导出器会自动创建父目录）
        let mut csv_exporter = CsvExporter::new(&self.data_file, true);
        csv_exporter.initialize()?;
        self.csv_exporter = Some(csv_exporter);

        Ok(())
    }

    fn export(&mut self, sqllog: &Sqllog<'_>) -> Result<()> {
        let csv_exporter = self.csv_exporter.as_mut().ok_or_else(|| {
            Error::Export(ExportError::IoError {
                path: self.data_file.clone().into(),
                reason: "CSV exporter not initialized".to_string(),
            })
        })?;

        csv_exporter.export(sqllog)?;
        Ok(())
    }

    fn export_batch(&mut self, sqllogs: &[&Sqllog<'_>]) -> Result<()> {
        let csv_exporter = self.csv_exporter.as_mut().ok_or_else(|| {
            Error::Export(ExportError::IoError {
                path: self.data_file.clone().into(),
                reason: "CSV exporter not initialized".to_string(),
            })
        })?;

        // 使用 CSV 导出器的并行批量处理
        csv_exporter.export_batch(sqllogs)?;
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        // 完成 CSV 导出
        if let Some(mut csv_exporter) = self.csv_exporter.take() {
            csv_exporter.finalize()?;

            // 获取统计信息
            if let Some(stats) = csv_exporter.stats_snapshot() {
                info!("CSV export completed: {} records", stats.exported);
            }
        }

        // 确保 log_dir 存在
        std::fs::create_dir_all(&self.log_dir).map_err(|e| {
            Error::Export(ExportError::IoError {
                path: self.log_dir.clone().into(),
                reason: e.to_string(),
            })
        })?;

        // 生成控制文件
        self.generate_control_file()?;

        // 创建表（如果不存在）
        self.create_table_if_not_exists()?;

        // 运行 dmfldr
        info!("Running dmfldr...");
        let log_file = Path::new(&self.log_dir).join("dmfldr.log");
        let control_file_abs = std::fs::canonicalize(&self.control_file)
            .unwrap_or_else(|_| PathBuf::from(&self.control_file))
            .display()
            .to_string()
            .replace(r"\\?\", "")
            .replace("\\", "/");

        let log_file_str = log_file.display().to_string().replace("\\", "/");

        // dmfldr USERID=SYSDBA/SYSDBA@localhost:5236 CONTROL='export/sqllog.ctl' LOG='export/log/dmfldr.log' SKIP=1
        // 注意：dmfldr 的第一个参数必须是 USERID，且字符串参数需要用引号
        let output = Command::new("dmfldr")
            .arg(self.userid.clone())
            .arg(format!("CONTROL='{}'", control_file_abs))
            .arg(format!("LOG='{}'", log_file_str))
            .arg("SKIP=1")
            .output();

        match output {
            Ok(o) => {
                if o.status.success() {
                    info!("dmfldr completed successfully.");
                    info!("Output: {}", String::from_utf8_lossy(&o.stdout));
                } else {
                    let err_msg = String::from_utf8_lossy(&o.stderr);
                    let out_msg = String::from_utf8_lossy(&o.stdout);
                    return Err(Error::Export(ExportError::ExternalToolError {
                        tool: "dmfldr".to_string(),
                        reason: format!(
                            "Exit code: {:?}\nStdout: {}\nStderr: {}",
                            o.status.code(),
                            out_msg,
                            err_msg
                        ),
                    }));
                }
            }
            Err(e) => {
                return Err(Error::Export(ExportError::ExternalToolError {
                    tool: "dmfldr".to_string(),
                    reason: format!("Failed to execute dmfldr: {}", e),
                }));
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "DM (dmfldr)"
    }

    fn stats_snapshot(&self) -> Option<ExportStats> {
        self.csv_exporter.as_ref()?.stats_snapshot()
    }
}

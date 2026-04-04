use crate::color;
use crate::config::Config;
use std::path::Path;

/// 在 run 命令执行前检查基础条件。
/// 返回所有警告/错误，调用方决定是否中止。
#[must_use]
pub fn check(cfg: &Config) -> PreflightResult {
    let mut result = PreflightResult::default();
    check_log_dir(&cfg.sqllog.directory, &mut result);
    check_output_writable(cfg, &mut result);
    result
}

fn check_log_dir(directory: &str, result: &mut PreflightResult) {
    let path = Path::new(directory);
    if !path.exists() {
        result.errors.push(format!(
            "日志目录不存在: {directory}  (可用 --set sqllog.directory=<path> 覆盖)"
        ));
        return;
    }
    if !path.is_dir() {
        result.errors.push(format!("路径不是目录: {directory}"));
        return;
    }

    let has_logs = std::fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .any(|e| e.path().extension().is_some_and(|x| x == "log"))
        })
        .unwrap_or(false);

    if !has_logs {
        result
            .warnings
            .push(format!("日志目录 {directory} 中未找到 .log 文件"));
    }
}

#[allow(clippy::needless_return)]
fn check_output_writable(cfg: &Config, result: &mut PreflightResult) {
    #[cfg(feature = "csv")]
    if let Some(csv) = &cfg.exporter.csv {
        check_path_writable(&csv.file, result);
        return;
    }
    #[cfg(feature = "jsonl")]
    if let Some(jsonl) = &cfg.exporter.jsonl {
        check_path_writable(&jsonl.file, result);
        return;
    }
    #[cfg(feature = "sqlite")]
    if let Some(sqlite) = &cfg.exporter.sqlite {
        check_path_writable(&sqlite.database_url, result);
        return;
    }
    #[cfg(not(any(feature = "csv", feature = "jsonl", feature = "sqlite")))]
    let _ = cfg;
}

fn check_path_writable(file_path: &str, result: &mut PreflightResult) {
    let path = Path::new(file_path);

    if path.exists() {
        if std::fs::OpenOptions::new().append(true).open(path).is_err() {
            result.errors.push(format!("输出文件不可写: {file_path}"));
        }
        return;
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    if parent.as_os_str().is_empty() || parent == Path::new(".") {
        return;
    }

    if parent.exists() {
        let tmp = parent.join(".sqllog2db_preflight_check");
        match std::fs::File::create(&tmp) {
            Ok(_) => {
                let _ = std::fs::remove_file(&tmp);
            }
            Err(_) => {
                result
                    .errors
                    .push(format!("输出目录不可写: {}", parent.display()));
            }
        }
    } else if std::fs::create_dir_all(parent).is_err() {
        result
            .errors
            .push(format!("无法创建输出目录: {}", parent.display()));
    }
}

#[derive(Debug, Default)]
pub struct PreflightResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl PreflightResult {
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 打印所有警告和错误，返回是否有致命错误。
    #[must_use]
    pub fn print_and_check(&self) -> bool {
        for warn in &self.warnings {
            eprintln!("{} {warn}", color::yellow("Warning:"));
        }
        for err in &self.errors {
            eprintln!("{} {err}", color::red("Error:"));
        }
        self.has_errors()
    }
}

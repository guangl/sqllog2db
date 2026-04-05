use crate::color;
use crate::config::Config;
use crate::parser::SqllogParser;
use std::path::Path;

/// 在 run 命令执行前检查基础条件。
/// 返回所有警告/错误，调用方决定是否中止。
#[must_use]
pub fn check(cfg: &Config) -> PreflightResult {
    let mut result = PreflightResult::default();
    check_log_path(&cfg.sqllog.path, &mut result);
    check_output_writable(cfg, &mut result);
    result
}

fn check_log_path(path_str: &str, result: &mut PreflightResult) {
    let has_glob = path_str.contains('*') || path_str.contains('?') || path_str.contains('[');

    // For non-glob paths, check existence before trying to scan
    if !has_glob {
        let path = Path::new(path_str);
        if !path.exists() {
            result.errors.push(format!(
                "日志路径不存在: {path_str}  (可用 --set sqllog.path=<path> 覆盖)"
            ));
            return;
        }
    }

    match SqllogParser::new(path_str).log_files() {
        Ok(files) if files.is_empty() => {
            result
                .warnings
                .push(format!("路径 {path_str} 中未找到 .log 文件"));
        }
        Ok(_) => {}
        Err(e) => {
            result.errors.push(format!("扫描日志路径失败: {e}"));
        }
    }
}

#[allow(clippy::needless_return)]
fn check_output_writable(cfg: &Config, result: &mut PreflightResult) {
    if let Some(csv) = &cfg.exporter.csv {
        check_path_writable(&csv.file, result);
        return;
    }
    if let Some(sqlite) = &cfg.exporter.sqlite {
        check_path_writable(&sqlite.database_url, result);
        return;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, CsvExporter, ExporterConfig, SqllogConfig};

    fn config_with_log_dir(dir: &str) -> Config {
        Config {
            sqllog: SqllogConfig {
                path: dir.to_string(),
            },
            ..Default::default()
        }
    }

    // ── PreflightResult ───────────────────────────────────────────

    #[test]
    fn test_preflight_result_no_errors() {
        let result = PreflightResult::default();
        assert!(!result.has_errors());
        assert!(!result.print_and_check());
    }

    #[test]
    fn test_preflight_result_with_errors() {
        let mut result = PreflightResult::default();
        result.errors.push("some error".to_string());
        assert!(result.has_errors());
        assert!(result.print_and_check());
    }

    #[test]
    fn test_preflight_result_warnings_no_error() {
        let mut result = PreflightResult::default();
        result.warnings.push("some warning".to_string());
        assert!(!result.has_errors());
        assert!(!result.print_and_check());
    }

    // ── check: log dir ────────────────────────────────────────────

    #[test]
    fn test_check_nonexistent_log_dir_produces_error() {
        let cfg = config_with_log_dir("/this/path/definitely/does/not/exist");
        let result = check(&cfg);
        assert!(result.has_errors());
        assert!(result.errors[0].contains("不存在"));
    }

    #[test]
    fn test_check_single_log_file_is_valid() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("test.log");
        std::fs::write(&file_path, "").unwrap();
        let cfg = config_with_log_dir(file_path.to_str().unwrap());
        let result = check(&cfg);
        assert!(!result.has_errors());
    }

    #[test]
    fn test_check_log_dir_empty_produces_warning() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = config_with_log_dir(dir.path().to_str().unwrap());
        let result = check(&cfg);
        assert!(!result.has_errors());
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_check_log_dir_with_log_files_no_warning() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.log"), "").unwrap();
        let cfg = config_with_log_dir(dir.path().to_str().unwrap());
        let result = check(&cfg);
        assert!(!result.has_errors());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_check_glob_pattern_with_matches() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.log"), "").unwrap();
        let pattern = format!("{}/*.log", dir.path().display());
        let cfg = config_with_log_dir(&pattern);
        let result = check(&cfg);
        assert!(!result.has_errors());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_check_glob_pattern_no_matches_produces_warning() {
        let dir = tempfile::TempDir::new().unwrap();
        let pattern = format!("{}/nomatch*.log", dir.path().display());
        let cfg = config_with_log_dir(&pattern);
        let result = check(&cfg);
        assert!(!result.has_errors());
        assert!(!result.warnings.is_empty());
    }

    // ── check: output writable ────────────────────────────────────

    #[test]
    fn test_check_csv_output_in_existing_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.log"), "").unwrap();
        let out_file = dir.path().join("out.csv");
        let mut cfg = config_with_log_dir(dir.path().to_str().unwrap());
        cfg.exporter = ExporterConfig {
            csv: Some(CsvExporter {
                file: out_file.to_str().unwrap().to_string(),
                overwrite: false,
                append: false,
            }),
            ..Default::default()
        };
        let result = check(&cfg);
        assert!(!result.has_errors());
    }

    #[test]
    fn test_check_csv_existing_writable_file() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.log"), "").unwrap();
        let out_file = dir.path().join("out.csv");
        std::fs::write(&out_file, "").unwrap(); // pre-create file
        let mut cfg = config_with_log_dir(dir.path().to_str().unwrap());
        cfg.exporter = ExporterConfig {
            csv: Some(CsvExporter {
                file: out_file.to_str().unwrap().to_string(),
                overwrite: false,
                append: false,
            }),
            ..Default::default()
        };
        let result = check(&cfg);
        assert!(!result.has_errors());
    }
}

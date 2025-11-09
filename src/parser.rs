/// SQL 日志解析模块
/// 使用 dm-database-parser-sqllog 库解析达梦数据库的 SQL 日志文件
use crate::error::{Error, ParserError, Result};
use dm_database_parser_sqllog::{Sqllog, parse_sqllogs_from_file};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// SQL 日志解析器
pub struct SqllogParser {
    /// 日志路径（文件或目录）
    path: PathBuf,
    /// 线程数（0 表示自动）
    thread_count: usize,
}

impl SqllogParser {
    /// 创建新的解析器
    pub fn new(path: impl AsRef<Path>, thread_count: usize) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            thread_count,
        }
    }

    /// 计算实际使用的线程数
    /// 逻辑需求：按文件数量创建线程（一个文件一个线程），若超过最大 CPU 核心数则使用最大 CPU 核心数。
    /// 若用户指定 thread_count > 0，则作为上限与 CPU/文件数共同裁剪。
    /// file_count = 0 时返回 0。
    fn calc_thread_count(&self, file_count: usize) -> usize {
        if file_count == 0 {
            return 0;
        }

        let max_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        let threads = if self.thread_count == 0 {
            // 自动模式：文件数与 CPU 数取较小值
            file_count.min(max_cpus)
        } else {
            // 手动模式：用户指定值再与文件数/CPU数裁剪
            self.thread_count.min(file_count).min(max_cpus)
        };

        // 至少 1（在 file_count>0 情况下）
        std::cmp::max(1, threads)
    }

    /// 扫描并获取所有需要解析的日志文件
    fn scan_log_files(&self) -> Result<Vec<PathBuf>> {
        let path = &self.path;

        if !path.exists() {
            return Err(Error::Parser(ParserError::PathNotFound {
                path: path.clone(),
            }));
        }

        let mut log_files = Vec::new();

        if path.is_file() {
            // 单个文件
            info!("解析单个日志文件: {}", path.display());
            log_files.push(path.clone());
        } else if path.is_dir() {
            // 目录：扫描所有 .log 文件
            info!("扫描日志目录: {}", path.display());

            let entries = std::fs::read_dir(path).map_err(|e| {
                Error::Parser(ParserError::ReadDirFailed {
                    path: path.clone(),
                    reason: e.to_string(),
                })
            })?;

            for entry in entries {
                let entry = entry.map_err(|e| {
                    Error::Parser(ParserError::ReadDirFailed {
                        path: path.clone(),
                        reason: e.to_string(),
                    })
                })?;

                let entry_path = entry.path();

                // 只处理 .log 文件
                if entry_path.is_file() {
                    if let Some(ext) = entry_path.extension() {
                        if ext == "log" {
                            debug!("发现日志文件: {}", entry_path.display());
                            log_files.push(entry_path);
                        }
                    }
                }
            }

            if log_files.is_empty() {
                warn!("目录 {} 中没有找到 .log 文件", path.display());
            } else {
                info!("找到 {} 个日志文件", log_files.len());
            }
        } else {
            return Err(Error::Parser(ParserError::InvalidPath {
                path: path.clone(),
                reason: "既不是文件也不是目录".to_string(),
            }));
        }

        Ok(log_files)
    }

    /// 解析单个日志文件（抽取为独立函数）
    fn parse_single_file(
        file_path: &Path,
    ) -> Result<(Vec<Sqllog>, Vec<dm_database_parser_sqllog::ParseError>)> {
        let path_str = file_path.to_string_lossy().to_string();
        info!("解析文件: {}", file_path.display());

        let (sqllogs, errors) = parse_sqllogs_from_file(&path_str).map_err(|e| {
            Error::Parser(ParserError::ParseFailed {
                reason: format!("解析文件 {} 失败: {:?}", file_path.display(), e),
            })
        })?;

        debug!(
            "文件 {} 解析到 {} 条记录，{} 个解析错误",
            file_path.display(),
            sqllogs.len(),
            errors.len()
        );

        Ok((sqllogs, errors))
    }

    /// 解析所有日志文件
    pub fn parse(&self) -> Result<Vec<Sqllog>> {
        let log_files = self.scan_log_files()?;

        if log_files.is_empty() {
            return Ok(Vec::new());
        }

        let thread_count = self.calc_thread_count(log_files.len());
        info!(
            "使用 {} 个线程解析 {} 个日志文件",
            thread_count,
            log_files.len()
        );

        let mut all_sqllogs: Vec<Sqllog> = Vec::new();
        let mut all_parse_errors = Vec::new();

        if thread_count <= 1 || log_files.len() == 1 {
            // 单线程路径
            for file_path in &log_files {
                let (sqllogs, errors) = Self::parse_single_file(file_path)?;
                all_sqllogs.extend(sqllogs);
                all_parse_errors.extend(errors);
            }
        } else {
            // 使用 rayon 并行解析，自定义线程池以控制线程数上限
            use rayon::prelude::*;

            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(thread_count)
                .build()
                .map_err(|e| {
                    Error::Parser(ParserError::ParseFailed {
                        reason: format!("创建线程池失败: {}", e),
                    })
                })?;

            let results: Result<Vec<(Vec<Sqllog>, Vec<_>)>> = pool.install(|| {
                log_files
                    .par_iter()
                    .map(|file_path| Self::parse_single_file(file_path))
                    .collect()
            });

            match results {
                Ok(parsed_files) => {
                    for (sqllogs, errors) in parsed_files {
                        all_sqllogs.extend(sqllogs);
                        all_parse_errors.extend(errors);
                    }
                }
                Err(e) => return Err(e),
            }
        }

        // 如果有解析错误，记录警告
        if !all_parse_errors.is_empty() {
            warn!("解析过程中遇到 {} 个错误（已跳过）", all_parse_errors.len());
            for (i, err) in all_parse_errors.iter().take(5).enumerate() {
                warn!("  错误 #{}: {:?}", i + 1, err);
            }
            if all_parse_errors.len() > 5 {
                warn!("  ... 还有 {} 个错误未显示", all_parse_errors.len() - 5);
            }
        }

        info!("成功解析 {} 条 SQL 日志记录", all_sqllogs.len());

        Ok(all_sqllogs)
    }

    /// 获取日志路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 获取线程数配置
    pub fn thread_count(&self) -> usize {
        self.thread_count
    }

    /// 流式解析：对每个解析出的 Sqllog 调用回调函数，避免一次性加载全部到内存。
    /// 回调返回 Err 则提前终止解析并返回该错误。
    /// 同时支持错误回调，处理解析失败的记录
    pub fn parse_with<F, E>(&self, mut on_record: F, mut on_error: E) -> Result<()>
    where
        F: FnMut(&Sqllog) -> Result<()>,
        E: FnMut(&Path, &dm_database_parser_sqllog::ParseError) -> Result<()>,
    {
        let log_files = self.scan_log_files()?;
        if log_files.is_empty() {
            return Ok(());
        }
        let thread_count = self.calc_thread_count(log_files.len());
        info!(
            "流式解析: {} 个文件, 使用线程 {}",
            log_files.len(),
            thread_count
        );

        // 与原实现类似，但不聚合所有记录；并行时仍需收集后再回放（rayon par_iter 不支持回调提前短路易处理）
        if thread_count <= 1 || log_files.len() == 1 {
            for file in &log_files {
                let (sqllogs, errors) = Self::parse_single_file(file)?;
                for s in &sqllogs {
                    on_record(s)?;
                }
                if !errors.is_empty() {
                    warn!("文件 {} 存在 {} 个错误", file.display(), errors.len());
                    for err in &errors {
                        on_error(file, err)?;
                    }
                }
            }
        } else {
            use rayon::prelude::*;
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(thread_count)
                .build()
                .map_err(|e| {
                    Error::Parser(ParserError::ParseFailed {
                        reason: format!("创建线程池失败: {}", e),
                    })
                })?;
            let results: Result<Vec<(PathBuf, Vec<Sqllog>, Vec<_>)>> = pool.install(|| {
                log_files
                    .par_iter()
                    .map(|f| {
                        Self::parse_single_file(f).map(|(sqllogs, errors)| (f.clone(), sqllogs, errors))
                    })
                    .collect()
            });
            match results {
                Ok(parsed) => {
                    for (file_path, sqllogs, errors) in parsed {
                        for s in &sqllogs {
                            on_record(s)?;
                        }
                        if !errors.is_empty() {
                            warn!("文件 {} 解析出现 {} 个错误", file_path.display(), errors.len());
                            for err in &errors {
                                on_error(&file_path, err)?;
                            }
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn create_test_log_file(path: &Path, content: &str) {
        let mut file = fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_parser_new() {
        let parser = SqllogParser::new("test.log", 4);
        assert_eq!(parser.path(), Path::new("test.log"));
        assert_eq!(parser.thread_count(), 4);
    }

    #[test]
    fn test_calc_thread_count_auto_less_than_cpu() {
        // 创建临时目录和 2 个文件，假设 CPU >=2
        let temp_dir = std::env::temp_dir().join("test_calc_threads_less");
        let _ = fs::create_dir(&temp_dir);
        create_test_log_file(&temp_dir.join("a.log"), "a");
        create_test_log_file(&temp_dir.join("b.log"), "b");
        let parser = SqllogParser::new(&temp_dir, 0);
        let files = parser.scan_log_files().unwrap();
        let cpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let tc = parser.calc_thread_count(files.len());
        assert_eq!(tc, files.len().min(cpu));
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_calc_thread_count_auto_more_than_cpu() {
        // 构造一个较大的 file_count (模拟，无需真实文件) 直接调用 calc_thread_count
        let parser = SqllogParser::new("dummy", 0);
        let cpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let big_count = cpu + 10;
        let tc = parser.calc_thread_count(big_count);
        assert_eq!(tc, cpu); // 超过 CPU，裁剪到 CPU
    }

    #[test]
    fn test_calc_thread_count_manual_upper_bound() {
        // 用户指定 8，但文件只有 3 个，应裁剪为 3 或 CPU 更小值
        let parser = SqllogParser::new("dummy", 8);
        let cpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let tc = parser.calc_thread_count(3);
        assert_eq!(tc, 8.min(3).min(cpu));
    }

    #[test]
    fn test_calc_thread_count_zero_files() {
        let parser = SqllogParser::new("dummy", 0);
        assert_eq!(parser.calc_thread_count(0), 0);
    }

    #[test]
    fn test_scan_log_files_nonexistent_path() {
        let parser = SqllogParser::new("nonexistent_path_12345", 1);
        let result = parser.scan_log_files();
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.to_string().contains("路径不存在"));
        }
    }

    #[test]
    fn test_scan_log_files_single_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_scan_single.log");

        create_test_log_file(&test_file, "test content");

        let parser = SqllogParser::new(&test_file, 1);
        let result = parser.scan_log_files();

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], test_file);

        // 清理
        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_scan_log_files_directory() {
        let temp_dir = std::env::temp_dir().join("test_scan_dir");
        let _ = fs::create_dir(&temp_dir);

        // 创建几个测试文件
        create_test_log_file(&temp_dir.join("file1.log"), "log 1");
        create_test_log_file(&temp_dir.join("file2.log"), "log 2");
        create_test_log_file(&temp_dir.join("file3.txt"), "not a log");

        let parser = SqllogParser::new(&temp_dir, 1);
        let result = parser.scan_log_files();

        assert!(result.is_ok());
        let files = result.unwrap();

        // 应该只找到 2 个 .log 文件
        assert_eq!(files.len(), 2);

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scan_log_files_empty_directory() {
        let temp_dir = std::env::temp_dir().join("test_scan_empty");
        let _ = fs::create_dir(&temp_dir);

        let parser = SqllogParser::new(&temp_dir, 1);
        let result = parser.scan_log_files();

        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 0);

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // 注意：parse() 方法需要真实的达梦日志格式，
    // 这里只测试基础功能，实际解析测试依赖于 dm-database-parser-sqllog 库
    #[test]
    fn test_parse_nonexistent_path() {
        let parser = SqllogParser::new("nonexistent_path_99999", 1);
        let result = parser.parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_directory() {
        let temp_dir = std::env::temp_dir().join("test_parse_empty");
        let _ = fs::create_dir(&temp_dir);

        let parser = SqllogParser::new(&temp_dir, 1);
        let result = parser.parse();

        assert!(result.is_ok());
        let sqllogs = result.unwrap();
        assert_eq!(sqllogs.len(), 0);

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }
}

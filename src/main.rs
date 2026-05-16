use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod charts;
mod cli;
mod color;
mod config;
mod error;
mod exporter;
mod features;
mod lang;
mod logging;
mod parser;
mod resume;

use config::Config;
use error::Result;
use log::{info, warn};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// 退出码约定：
// 0  = 成功
// 1  = 未分类错误
// 2  = 配置错误
// 3  = 输入/文件/解析错误
// 4  = 导出错误
// 130 = 被用户中断（Ctrl+C），遵循 Unix 128+SIGINT(2) 惯例
const EXIT_CONFIG: i32 = 2;
const EXIT_IO: i32 = 3;
const EXIT_EXPORT: i32 = 4;
const EXIT_INTERRUPTED: i32 = 130;

fn exit_code_for(e: &error::Error) -> i32 {
    match e {
        error::Error::Config(_) => EXIT_CONFIG,
        error::Error::File(_) | error::Error::Parser(_) | error::Error::Io(_) => EXIT_IO,
        error::Error::Export(_) => EXIT_EXPORT,
        error::Error::Interrupted => EXIT_INTERRUPTED,
        error::Error::Update(_) => 1,
    }
}

/// Initialize simple console logging for init/completions/update commands
fn init_simple_logging(verbose: bool, quiet: bool) {
    let level = if verbose {
        "debug"
    } else if quiet {
        "error"
    } else {
        "info"
    };

    let filter = match level {
        "debug" => log::LevelFilter::Debug,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };

    let _ = env_logger::Builder::from_default_env()
        .filter_level(filter)
        .try_init();
}

/// Apply CLI flags (verbose/quiet) to configuration
fn apply_cli_flags_to_config(cfg: &mut Config, verbose: bool, quiet: bool) {
    if verbose {
        cfg.logging.level = "debug".to_string();
    } else if quiet {
        cfg.logging.level = "error".to_string();
    }
}

/// Apply --from / --to date range to filters config
fn apply_date_range(cfg: &mut Config, from: Option<&str>, to: Option<&str>) {
    if from.is_none() && to.is_none() {
        return;
    }
    let filters = cfg.features.filters.get_or_insert_with(Default::default);
    filters.enable = true;
    if let Some(f) = from {
        filters.meta.start_ts = Some(f.to_string());
    }
    if let Some(t) = to {
        filters.meta.end_ts = Some(t.to_string());
    }
}

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => {
            let code = exit_code_for(&e);
            // Interrupted：静默退出，进度条已清除，用户清楚自己按了 Ctrl+C
            if code != EXIT_INTERRUPTED {
                eprintln!("{} {e}", color::red("Error:"));
            }
            std::process::exit(code);
        }
    }
}

fn run() -> Result<()> {
    use clap::{CommandFactory, FromArgMatches, Parser};

    // Pre-scan raw args to detect language before building the command, so
    // that `--help` output is already in the right language.
    let raw_args: Vec<String> = std::env::args().collect();
    let lang = lang::detect(&raw_args);

    let base_cmd = cli::opts::Cli::command();
    let cmd = if lang == lang::Lang::Zh {
        lang::apply_zh(base_cmd)
    } else {
        base_cmd
    };
    let matches = cmd.get_matches();
    let cli = cli::opts::Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    // 尽早初始化颜色开关，后续所有输出均依赖此状态
    color::init(cli.no_color);

    // run/stats/digest 命令不走 env_logger，避免与进度条冲突；其他命令用 env_logger 输出到终端
    let needs_simple_logging = !matches!(
        &cli.command,
        Some(
            cli::opts::Commands::Run { .. }
                | cli::opts::Commands::Stats { .. }
                | cli::opts::Commands::Digest { .. }
        )
    );
    if needs_simple_logging {
        init_simple_logging(cli.verbose, cli.quiet);
    }

    // Check for updates at startup unless we are already running self-update or quiet
    if !cli.quiet
        && !matches!(
            &cli.command,
            Some(cli::opts::Commands::SelfUpdate { .. } | cli::opts::Commands::Completions { .. })
        )
    {
        cli::update::check_for_updates_at_startup();
    }

    match &cli.command {
        Some(cli::opts::Commands::Init { output, force }) => {
            cli::init::handle_init(output, *force, lang)
        }
        Some(cli::opts::Commands::Completions { shell }) => {
            cli::opts::Cli::generate_completions(*shell);
            Ok(())
        }
        Some(cli::opts::Commands::SelfUpdate { check }) => cli::update::handle_update(*check),
        Some(cli::opts::Commands::Man) => {
            use clap::CommandFactory;
            let cmd = cli::opts::Cli::command();
            let man = clap_mangen::Man::new(cmd);
            man.render(&mut std::io::stdout())?;
            Ok(())
        }
        Some(cli::opts::Commands::Run {
            config,
            limit,
            dry_run,
            set,
            from,
            to,
            output,
            progress_interval,
            resume,
            state_file,
            jobs,
        }) => {
            let mut cfg = load_config(config)?;
            // --output is a shorthand applied before --set so --set can override
            let mut all_set = Vec::new();
            if let Some(out) = output {
                all_set.push(format!("exporter.csv.file={out}"));
            }
            all_set.extend_from_slice(set);
            cfg.apply_overrides(&all_set)?;
            apply_date_range(&mut cfg, from.as_deref(), to.as_deref());
            // 替换：validate() → validate_and_compile()，消除 run 路径中的双重 regex 编译（SC-2）
            let compiled_filters = cfg.validate_and_compile()?;

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            // run 命令使用进度条，日志只写文件不写 stdout
            logging::init_logging(&cfg.logging, false)?;
            info!("Application started");
            info!("Configuration validation passed");

            // preflight：日志目录 + 输出可写性
            if !*dry_run {
                let pf = cli::preflight::check(&cfg);
                if pf.print_and_check() {
                    std::process::exit(EXIT_CONFIG);
                }
            }

            // 注册 Ctrl+C 处理器：设置中断标志，让处理循环在下一个 batch 结束时优雅退出
            let interrupted = Arc::new(AtomicBool::new(false));
            let interrupted_flag = Arc::clone(&interrupted);
            ctrlc::set_handler(move || {
                interrupted_flag.store(true, Ordering::Relaxed);
            })
            .ok();

            let jobs = jobs.unwrap_or_else(|| {
                std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
            });
            cli::run::handle_run(
                &cfg,
                *limit,
                *dry_run,
                cli.quiet,
                &interrupted,
                *progress_interval,
                *resume,
                state_file.as_deref(),
                jobs,
                compiled_filters, // 新增：传递预编译结果
            )
        }
        Some(cli::opts::Commands::Validate { config, set }) => {
            let mut cfg = load_config(config)?;
            cfg.apply_overrides(set)?;
            cfg.validate()?;

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            // validate 命令无进度条，日志同时输出到 stdout
            logging::init_logging(&cfg.logging, true)?;
            info!("Application started");
            info!("Configuration validation passed");

            cli::validate::handle_validate(&cfg);
            Ok(())
        }
        Some(cli::opts::Commands::ShowConfig { config, set, diff }) => {
            let mut cfg = load_config(config)?;
            cfg.apply_overrides(set)?;
            cli::show_config::handle_show_config(&cfg, config, *diff);
            Ok(())
        }
        Some(cli::opts::Commands::Stats {
            config,
            set,
            from,
            to,
            top,
            json,
            group_by,
            bucket,
            resume,
            state_file,
        }) => {
            let mut cfg = load_config(config)?;
            cfg.apply_overrides(set)?;
            apply_date_range(&mut cfg, from.as_deref(), to.as_deref());
            let resume_state_file = if *resume {
                Some(
                    state_file
                        .as_deref()
                        .unwrap_or(cli::stats::DEFAULT_STATS_STATE),
                )
            } else {
                None
            };
            cli::stats::handle_stats(
                &cfg,
                cli.quiet,
                cli.verbose,
                *top,
                *json,
                group_by,
                bucket.as_deref(),
                resume_state_file,
            );
            Ok(())
        }
        Some(cli::opts::Commands::Digest {
            config,
            set,
            from,
            to,
            top,
            sort,
            min_count,
            json,
            resume,
            state_file,
        }) => {
            let mut cfg = load_config(config)?;
            cfg.apply_overrides(set)?;
            apply_date_range(&mut cfg, from.as_deref(), to.as_deref());
            let Some(sort_by) = cli::digest::SortBy::parse(sort) else {
                eprintln!(
                    "{} Unknown sort field '{}'. Valid values: count, exec",
                    color::red("Error:"),
                    sort
                );
                std::process::exit(EXIT_CONFIG);
            };
            let resume_state_file = if *resume {
                Some(
                    state_file
                        .as_deref()
                        .unwrap_or(cli::digest::DEFAULT_DIGEST_STATE),
                )
            } else {
                None
            };
            cli::digest::handle_digest(
                &cfg,
                cli.quiet,
                *top,
                sort_by,
                *min_count,
                *json,
                resume_state_file,
            );
            Ok(())
        }
        None => {
            let _ = cli::opts::Cli::try_parse_from(["sqllog2db", "--help"]);
            std::process::exit(1);
        }
    }
}

fn load_config(config_path: &str) -> Result<Config> {
    let path = Path::new(config_path);
    match Config::from_file(path) {
        Ok(c) => {
            info!("Loaded configuration file: {config_path}");
            Ok(c)
        }
        Err(e) => {
            if let error::Error::Config(error::ConfigError::NotFound(_)) = &e {
                warn!("Configuration file not found: {config_path}, using default configuration");
                info!("Tip: run 'sqllog2db init' to generate a configuration file");
                Ok(Config::default())
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ConfigError, ExportError, FileError, ParserError, UpdateError};

    #[test]
    fn test_exit_code_config_error() {
        let e = error::Error::Config(ConfigError::NoExporters);
        assert_eq!(exit_code_for(&e), EXIT_CONFIG);
    }

    #[test]
    fn test_exit_code_file_error() {
        let e = error::Error::File(FileError::CreateDirectoryFailed {
            path: "/tmp".into(),
            reason: "test".into(),
        });
        assert_eq!(exit_code_for(&e), EXIT_IO);
    }

    #[test]
    fn test_exit_code_parser_error() {
        let e = error::Error::Parser(ParserError::PathNotFound {
            path: "/tmp".into(),
        });
        assert_eq!(exit_code_for(&e), EXIT_IO);
    }

    #[test]
    fn test_exit_code_io_error() {
        let e = error::Error::Io(std::io::Error::other("test io"));
        assert_eq!(exit_code_for(&e), EXIT_IO);
    }

    #[test]
    fn test_exit_code_export_error() {
        let e = error::Error::Export(ExportError::DatabaseFailed {
            reason: "test".into(),
        });
        assert_eq!(exit_code_for(&e), EXIT_EXPORT);
    }

    #[test]
    fn test_exit_code_interrupted() {
        assert_eq!(exit_code_for(&error::Error::Interrupted), EXIT_INTERRUPTED);
    }

    #[test]
    fn test_exit_code_update_error() {
        let e = error::Error::Update(UpdateError::UpdateFailed("test".into()));
        assert_eq!(exit_code_for(&e), 1);
    }

    #[test]
    fn test_apply_cli_flags_verbose() {
        let mut cfg = Config::default();
        apply_cli_flags_to_config(&mut cfg, true, false);
        assert_eq!(cfg.logging.level, "debug");
    }

    #[test]
    fn test_apply_cli_flags_quiet() {
        let mut cfg = Config::default();
        apply_cli_flags_to_config(&mut cfg, false, true);
        assert_eq!(cfg.logging.level, "error");
    }

    #[test]
    fn test_apply_cli_flags_neither() {
        let mut cfg = Config::default();
        let original = cfg.logging.level.clone();
        apply_cli_flags_to_config(&mut cfg, false, false);
        assert_eq!(cfg.logging.level, original);
    }

    #[test]
    fn test_apply_date_range_both() {
        let mut cfg = Config::default();
        apply_date_range(&mut cfg, Some("2025-01-01"), Some("2025-12-31"));
        let f = cfg.features.filters.unwrap();
        assert_eq!(f.meta.start_ts.as_deref(), Some("2025-01-01"));
        assert_eq!(f.meta.end_ts.as_deref(), Some("2025-12-31"));
        assert!(f.enable);
    }

    #[test]
    fn test_apply_date_range_from_only() {
        let mut cfg = Config::default();
        apply_date_range(&mut cfg, Some("2025-06-01"), None);
        let f = cfg.features.filters.unwrap();
        assert_eq!(f.meta.start_ts.as_deref(), Some("2025-06-01"));
        assert!(f.meta.end_ts.is_none());
    }

    #[test]
    fn test_apply_date_range_to_only() {
        let mut cfg = Config::default();
        apply_date_range(&mut cfg, None, Some("2025-06-30"));
        let f = cfg.features.filters.unwrap();
        assert!(f.meta.start_ts.is_none());
        assert_eq!(f.meta.end_ts.as_deref(), Some("2025-06-30"));
    }

    #[test]
    fn test_apply_date_range_neither() {
        let mut cfg = Config::default();
        apply_date_range(&mut cfg, None, None);
        assert!(cfg.features.filters.is_none());
    }

    #[test]
    fn test_load_config_not_found_returns_default() {
        let result = load_config("/nonexistent/path/config.toml");
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_config_invalid_toml_returns_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "not valid toml ][[[").unwrap();
        let result = load_config(path.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_init_simple_logging_info() {
        // May silently fail if logger already set — that is fine
        init_simple_logging(false, false);
    }

    #[test]
    fn test_init_simple_logging_verbose() {
        init_simple_logging(true, false);
    }

    #[test]
    fn test_init_simple_logging_quiet() {
        init_simple_logging(false, true);
    }
}

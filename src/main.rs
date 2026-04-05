mod cli;
mod color;
mod config;
mod error;
mod exporter;
mod features;
mod logging;
mod parser;

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
    use clap::Parser;
    let cli = cli::opts::Cli::parse();

    // 尽早初始化颜色开关，后续所有输出均依赖此状态
    color::init(cli.no_color);

    // run/stats 命令不走 env_logger，避免与进度条冲突；其他命令用 env_logger 输出到终端
    let needs_simple_logging = !matches!(
        &cli.command,
        Some(cli::opts::Commands::Run { .. } | cli::opts::Commands::Stats { .. })
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
        Some(cli::opts::Commands::Init { output, force }) => cli::init::handle_init(output, *force),
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
            cfg.validate()?;
            info!("Configuration validation passed");

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            // run 命令使用进度条，日志只写文件不写 stdout
            logging::init_logging(&cfg.logging, false)?;
            info!("Application started");

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

            cli::run::handle_run(
                &cfg,
                *limit,
                *dry_run,
                cli.quiet,
                &interrupted,
                *progress_interval,
            )
        }
        Some(cli::opts::Commands::Validate { config, set }) => {
            let mut cfg = load_config(config)?;
            cfg.apply_overrides(set)?;
            cfg.validate()?;
            info!("Configuration validation passed");

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            // validate 命令无进度条，日志同时输出到 stdout
            logging::init_logging(&cfg.logging, true)?;
            info!("Application started");

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
        }) => {
            let mut cfg = load_config(config)?;
            cfg.apply_overrides(set)?;
            apply_date_range(&mut cfg, from.as_deref(), to.as_deref());
            cli::stats::handle_stats(&cfg, cli.quiet, cli.verbose, *top, *json);
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

mod cli;
mod color;
mod config;
mod constants;
mod error;
mod error_logger;
mod exporter;
mod features;
mod logging;
mod parser;

use config::Config;
use error::Result;
use log::{error, info, warn};
use std::path::Path;

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

/// Apply --from / --to date range to filters config (requires filters feature)
#[cfg(feature = "filters")]
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
    if let Err(e) = run() {
        error!("{e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    use clap::Parser;
    let cli = cli::opts::Cli::parse();

    // run 命令不走 env_logger，避免与进度条冲突；其他命令用 env_logger 输出到终端
    let is_run_command = matches!(&cli.command, Some(cli::opts::Commands::Run { .. }));
    if !is_run_command {
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
        Some(cli::opts::Commands::Run {
            config,
            limit,
            dry_run,
            set,
            from,
            to,
        }) => {
            let mut cfg = load_config(config)?;
            cfg.apply_overrides(set)?;

            #[cfg(feature = "filters")]
            apply_date_range(&mut cfg, from.as_deref(), to.as_deref());

            #[cfg(not(feature = "filters"))]
            if from.is_some() || to.is_some() {
                warn!("--from/--to require the 'filters' feature (ignored)");
            }

            cfg.validate()?;
            info!("Configuration validation passed");

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            // run 命令使用进度条，日志只写文件不写 stdout
            logging::init_logging(&cfg.logging, false)?;
            info!("Application started");

            cli::run::handle_run(&cfg, *limit, *dry_run)
        }
        Some(cli::opts::Commands::Validate { config }) => {
            let mut cfg = load_config(config)?;
            cfg.validate()?;
            info!("Configuration validation passed");

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            // validate 命令无进度条，日志同时输出到 stdout
            logging::init_logging(&cfg.logging, true)?;
            info!("Application started");

            cli::validate::handle_validate(&cfg);
            Ok(())
        }
        Some(cli::opts::Commands::ShowConfig { config, set }) => {
            let mut cfg = load_config(config)?;
            cfg.apply_overrides(set)?;
            cli::show_config::handle_show_config(&cfg, config);
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

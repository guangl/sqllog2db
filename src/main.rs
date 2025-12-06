mod cli;
mod config;
mod constants;
mod error;
mod error_logger;
mod exporter;
mod logging;
mod parser;

use config::Config;
use error::Result;
use log::info;
use std::path::Path;

/// Initialize simple console logging for init/completions commands
fn init_simple_logging(verbose: bool, quiet: bool) {
    let level = if verbose {
        "debug"
    } else if quiet {
        "error"
    } else {
        "info"
    };
    env_logger::Builder::from_default_env()
        .filter_level(match level {
            "debug" => log::LevelFilter::Debug,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        })
        .init();
}

/// Apply CLI flags (verbose/quiet) to configuration
fn apply_cli_flags_to_config(cfg: &mut Config, verbose: bool, quiet: bool) {
    if verbose {
        cfg.logging.level = "debug".to_string();
    } else if quiet {
        cfg.logging.level = "error".to_string();
    }
}

#[cfg(feature = "tui")]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    use clap::Parser;
    let cli = cli::opts::Cli::parse();

    match &cli.command {
        Some(cli::opts::Commands::Init { output, force }) => {
            init_simple_logging(cli.verbose, cli.quiet);
            cli::init::handle_init(output, *force)
        }
        Some(cli::opts::Commands::Completions { shell }) => {
            cli::opts::Cli::generate_completions(*shell);
            Ok(())
        }
        Some(cli::opts::Commands::Run { config, .. }) => {
            let mut cfg = load_config(config)?;
            cfg.validate()?;
            eprintln!("Configuration validation passed");

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            logging::init_logging(&cfg.logging)?;
            info!("Application started");

            #[cfg(feature = "tui")]
            if let Some(cli::opts::Commands::Run { use_tui, .. }) = &cli.command {
                if *use_tui {
                    // 在 TUI 模式下禁用控制台日志输出
                    logging::set_log_to_console(false);
                    return cli::run_tui::handle_run_tui(&cfg).await;
                }
            }

            cli::run::handle_run(&cfg)
        }
        Some(cli::opts::Commands::Validate { config }) => {
            let mut cfg = load_config(config)?;
            cfg.validate()?;
            eprintln!("Configuration validation passed");

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            logging::init_logging(&cfg.logging)?;
            info!("Application started");

            cli::validate::handle_validate(&cfg);
            Ok(())
        }
        None => {
            let _ = cli::opts::Cli::try_parse_from(["sqllog2db", "--help"]);
            std::process::exit(1);
        }
    }
}

#[cfg(not(feature = "tui"))]
fn main() -> Result<()> {
    use clap::Parser;
    let cli = cli::opts::Cli::parse();

    match &cli.command {
        Some(cli::opts::Commands::Init { output, force }) => {
            init_simple_logging(cli.verbose, cli.quiet);
            cli::init::handle_init(output, *force)
        }
        Some(cli::opts::Commands::Completions { shell }) => {
            cli::opts::Cli::generate_completions(*shell);
            Ok(())
        }
        Some(cli::opts::Commands::Run { config, .. }) => {
            let mut cfg = load_config(config)?;
            cfg.validate()?;
            eprintln!("Configuration validation passed");

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            logging::init_logging(&cfg.logging)?;
            info!("Application started");

            cli::run::handle_run(&cfg)
        }
        Some(cli::opts::Commands::Validate { config }) => {
            let mut cfg = load_config(config)?;
            cfg.validate()?;
            eprintln!("Configuration validation passed");

            apply_cli_flags_to_config(&mut cfg, cli.verbose, cli.quiet);
            logging::init_logging(&cfg.logging)?;
            info!("Application started");

            cli::validate::handle_validate(&cfg);
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
            eprintln!("Loaded configuration file: {config_path}");
            Ok(c)
        }
        Err(e) => {
            if let error::Error::Config(error::ConfigError::NotFound(_)) = &e {
                eprintln!(
                    "Configuration file not found: {config_path}, using default configuration"
                );
                eprintln!("Tip: run 'sqllog2db init' to generate a configuration file");
                Ok(Config::default())
            } else {
                Err(e)
            }
        }
    }
}

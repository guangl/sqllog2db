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

            if cli.verbose {
                cfg.logging.level = "debug".to_string();
            } else if cli.quiet {
                cfg.logging.level = "error".to_string();
            }

            logging::init_logging(&cfg.logging)?;
            info!("Application started");

            #[cfg(feature = "tui")]
            if let Some(cli::opts::Commands::Run { use_tui: true, .. }) = &cli.command {
                return cli::run_tui::handle_run_tui(&cfg).await;
            }

            cli::run::handle_run(&cfg)
        }
        Some(cli::opts::Commands::Validate { config }) => {
            let mut cfg = load_config(config)?;
            cfg.validate()?;
            eprintln!("Configuration validation passed");

            if cli.verbose {
                cfg.logging.level = "debug".to_string();
            } else if cli.quiet {
                cfg.logging.level = "error".to_string();
            }

            logging::init_logging(&cfg.logging)?;
            info!("Application started");

            cli::validate::handle_validate(&cfg)
        }
        None => {
            print_help();
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

            if cli.verbose {
                cfg.logging.level = "debug".to_string();
            } else if cli.quiet {
                cfg.logging.level = "error".to_string();
            }

            logging::init_logging(&cfg.logging)?;
            info!("Application started");

            cli::run::handle_run(&cfg)
        }
        Some(cli::opts::Commands::Validate { config }) => {
            let mut cfg = load_config(config)?;
            cfg.validate()?;
            eprintln!("Configuration validation passed");

            if cli.verbose {
                cfg.logging.level = "debug".to_string();
            } else if cli.quiet {
                cfg.logging.level = "error".to_string();
            }

            logging::init_logging(&cfg.logging)?;
            info!("Application started");

            cli::validate::handle_validate(&cfg)
        }
        None => {
            print_help();
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

fn print_help() {
    eprintln!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("sqllog2db - SQL Log Exporter for DM Database");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("\nUsage: sqllog2db <COMMAND> [OPTIONS]");
    eprintln!("\nCommands:");
    eprintln!("  run        Run the log export task");
    eprintln!("  init       Generate a default configuration file");
    eprintln!("  validate   Validate a configuration file");
    eprintln!("  complete   Generate shell completion scripts");
    eprintln!("\nOptions:");
    eprintln!("  -v, --verbose   Enable verbose output (debug level)");
    eprintln!("  -q, --quiet     Suppress non-error output");
    eprintln!("  -h, --help      Print help information");
    eprintln!("  -V, --version   Print version information");
    eprintln!("\nExamples:");
    eprintln!("  # Initialize configuration");
    eprintln!("  sqllog2db init");
    eprintln!("\n  # Run with default config");
    eprintln!("  sqllog2db run");
    eprintln!("\n  # Run with custom config and verbose logging");
    eprintln!("  sqllog2db -v run -c custom.toml");
    eprintln!("\n  # Validate configuration");
    eprintln!("  sqllog2db validate -c config.toml");
    #[cfg(feature = "tui")]
    {
        eprintln!("\n  # Run with TUI mode");
        eprintln!("  sqllog2db run --tui");
    }
    eprintln!("\nFor more help: sqllog2db --help");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
}

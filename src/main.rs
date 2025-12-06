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

fn main() -> Result<()> {
    // 解析命令行参数
    use clap::Parser;
    let cli = cli::opts::Cli::parse();

    // 根据命令类型决定是否需要加载配置
    match &cli.command {
        Some(cli::opts::Commands::Init { output, force }) => {
            // init 命令不需要加载配置,使用简单的控制台日志
            init_simple_logging(cli.verbose, cli.quiet);

            cli::init::handle_init(output, *force)
        }
        Some(cli::opts::Commands::Completions { shell }) => {
            // 生成 shell 补全脚本
            cli::opts::Cli::generate_completions(*shell);
            Ok(())
        }
        Some(cli::opts::Commands::Run { config })
        | Some(cli::opts::Commands::Validate { config }) => {
            // 加载配置,如果文件不存在则使用默认配置
            let path = Path::new(config);
            let mut cfg = match Config::from_file(path) {
                Ok(c) => {
                    eprintln!("Loaded configuration file: {}", config);
                    c
                }
                Err(e) => {
                    if let error::Error::Config(error::ConfigError::NotFound(_)) = &e {
                        eprintln!(
                            "Configuration file not found: {}, using default configuration",
                            config
                        );
                        eprintln!("Tip: run 'sqllog2db init' to generate a configuration file");
                        Config::default()
                    } else {
                        return Err(e);
                    }
                }
            };

            // 验证配置
            cfg.validate()?;
            eprintln!("Configuration validation passed");

            // Override log level based on CLI flags
            if cli.verbose {
                cfg.logging.level = "debug".to_string();
            } else if cli.quiet {
                cfg.logging.level = "error".to_string();
            }

            // 初始化日志系统
            logging::init_logging(&cfg.logging)?;
            info!("Application started");

            // 分发到具体命令
            match &cli.command {
                Some(cli::opts::Commands::Run { .. }) => cli::run::handle_run(&cfg),
                Some(cli::opts::Commands::Validate { .. }) => cli::validate::handle_validate(&cfg),
                _ => unreachable!(),
            }
        }
        None => {
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
            eprintln!("\nFor more help: sqllog2db --help");
            eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            std::process::exit(1);
        }
    }
}

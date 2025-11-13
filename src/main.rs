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

fn main() -> Result<()> {
    // 解析命令行参数
    use clap::Parser;
    let cli = cli::opts::Cli::parse();

    // 根据命令类型决定是否需要加载配置
    match &cli.command {
        Some(cli::opts::Commands::Init { output, force }) => {
            // init 命令不需要加载配置,使用简单的控制台日志
            env_logger::init();

            cli::init::handle_init(output, *force)
        }
        Some(cli::opts::Commands::Run { config })
        | Some(cli::opts::Commands::Validate { config }) => {
            // 加载配置,如果文件不存在则使用默认配置
            let path = Path::new(config);
            let cfg = match Config::from_file(path) {
                Ok(c) => {
                    eprintln!("已加载配置文件: {}", config);
                    c
                }
                Err(e) => {
                    if let error::Error::Config(error::ConfigError::NotFound(_)) = &e {
                        eprintln!("未找到配置文件: {},使用默认配置", config);
                        eprintln!("提示: 运行 'sqllog2db init' 生成配置文件");
                        Config::default()
                    } else {
                        return Err(e);
                    }
                }
            };

            // 验证配置
            cfg.validate()?;
            eprintln!("配置验证通过");

            // 初始化日志系统
            logging::init_logging(&cfg.logging)?;
            info!("应用程序启动");

            // 分发到具体命令
            match &cli.command {
                Some(cli::opts::Commands::Run { .. }) => cli::run::handle_run(&cfg),
                Some(cli::opts::Commands::Validate { .. }) => cli::validate::handle_validate(&cfg),
                _ => unreachable!(),
            }
        }
        None => {
            eprintln!("请指定子命令。使用 --help 查看可用命令。");
            std::process::exit(1);
        }
    }
}

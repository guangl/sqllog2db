use clap::{Parser, Subcommand};

/// SQL 日志导出工具
#[derive(Debug, Parser)]
#[command(name = "sqllog2db", version, about = "SQL 日志导出工具")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// 运行日志导出任务
    Run {
        /// 配置文件路径
        #[arg(short = 'c', long = "config", default_value = "config.toml")]
        config: String,
    },
    /// 生成默认配置文件
    Init {
        /// 输出配置文件路径
        #[arg(short = 'o', long = "output", default_value = "config.toml")]
        output: String,
        /// 强制覆盖已存在的文件
        #[arg(short = 'f', long = "force")]
        force: bool,
    },
    /// 验证配置文件
    Validate {
        /// 配置文件路径
        #[arg(short = 'c', long = "config", default_value = "config.toml")]
        config: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_run_command_default() {
        let args = ["sqllog2db", "run"];
        let cli = Cli::try_parse_from(&args).expect("parse run");
        match cli.command {
            Some(Commands::Run { config }) => {
                assert_eq!(config, "config.toml");
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn parse_run_command_with_config() {
        let args = ["sqllog2db", "run", "--config", "custom.toml"];
        let cli = Cli::try_parse_from(&args).expect("parse run with config");
        match cli.command {
            Some(Commands::Run { config }) => {
                assert_eq!(config, "custom.toml");
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn parse_init_command() {
        let args = ["sqllog2db", "init"];
        let cli = Cli::try_parse_from(&args).expect("parse init");
        match cli.command {
            Some(Commands::Init { output, force }) => {
                assert_eq!(output, "config.toml");
                assert!(!force);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn parse_init_command_with_force() {
        let args = ["sqllog2db", "init", "-o", "my_config.toml", "--force"];
        let cli = Cli::try_parse_from(&args).expect("parse init with force");
        match cli.command {
            Some(Commands::Init { output, force }) => {
                assert_eq!(output, "my_config.toml");
                assert!(force);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn parse_validate_command() {
        let args = ["sqllog2db", "validate", "-c", "test.toml"];
        let cli = Cli::try_parse_from(&args).expect("parse validate");
        match cli.command {
            Some(Commands::Validate { config }) => {
                assert_eq!(config, "test.toml");
            }
            _ => panic!("Expected Validate command"),
        }
    }
}

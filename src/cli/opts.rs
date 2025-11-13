use clap::{Parser, Subcommand};

/// SQL 日志导出工具
#[derive(Debug, Parser)]
#[command(name = "sqllog2db", version, about = "SQL log exporter tool")]
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

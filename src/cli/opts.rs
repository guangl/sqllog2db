use clap::{Parser, Subcommand};

/// SQL log exporter tool
#[derive(Debug, Parser)]
#[command(name = "sqllog2db", version, about = "SQL log exporter tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run the log export task
    Run {
        /// Configuration file path
        #[arg(short = 'c', long = "config", default_value = "config.toml")]
        config: String,
    },
    /// Generate a default configuration file
    Init {
        /// Output configuration file path
        #[arg(short = 'o', long = "output", default_value = "config.toml")]
        output: String,
        /// Force overwrite if file exists
        #[arg(short = 'f', long = "force")]
        force: bool,
    },
    /// Validate a configuration file
    Validate {
        /// Configuration file path
        #[arg(short = 'c', long = "config", default_value = "config.toml")]
        config: String,
    },
}

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};

/// SQL log exporter tool for DM database
#[derive(Debug, Parser)]
#[command(
    name = "sqllog2db",
    version,
    about = "Parse DM database SQL logs and export to CSV/Parquet/JSONL/SQLite/DuckDB/PostgreSQL/DM",
    long_about = "A lightweight and efficient CLI tool for parsing DM database SQL logs (streaming) and exporting to multiple formats with error tracking."
)]
pub struct Cli {
    /// Enable verbose output (debug level)
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    /// Suppress non-error output (error level only)
    #[arg(short = 'q', long = "quiet", global = true, conflicts_with = "verbose")]
    pub quiet: bool,

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
    /// Generate shell completion scripts
    Completions {
        /// Shell type to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

impl Cli {
    /// Generate shell completions
    pub fn generate_completions(shell: Shell) {
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    }
}

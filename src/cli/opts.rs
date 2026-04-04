use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};

/// SQL log exporter tool for DM database
#[derive(Debug, Parser)]
#[command(
    name = "sqllog2db",
    version,
    about = "Parse DM database SQL logs and export to CSV/JSONL/SQLite",
    long_about = "A lightweight and efficient CLI tool for parsing DM database SQL logs (streaming) and exporting to multiple formats with error tracking."
)]
pub struct Cli {
    /// Enable verbose output (debug level)
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    /// Suppress non-error output (error level only)
    #[arg(short = 'q', long = "quiet", global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Disable colored output (also respects `NO_COLOR` env var)
    #[arg(long = "no-color", global = true)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run the log export task
    Run {
        /// Configuration file path
        #[arg(
            short = 'c',
            long = "config",
            default_value = "config.toml",
            env = "SQLLOG2DB_CONFIG"
        )]
        config: String,
        /// Stop after processing N records (across all files)
        #[arg(short = 'n', long = "limit")]
        limit: Option<usize>,
        /// Parse and count records without writing output
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Override config values, e.g. --set exporter.csv.file=out.csv
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Keep only records at or after this timestamp (requires filters feature)
        #[arg(long = "from", value_name = "DATETIME")]
        from: Option<String>,
        /// Keep only records at or before this timestamp (requires filters feature)
        #[arg(long = "to", value_name = "DATETIME")]
        to: Option<String>,
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
        #[arg(
            short = 'c',
            long = "config",
            default_value = "config.toml",
            env = "SQLLOG2DB_CONFIG"
        )]
        config: String,
    },
    /// Show effective configuration (after loading and any --set overrides)
    ShowConfig {
        /// Configuration file path
        #[arg(
            short = 'c',
            long = "config",
            default_value = "config.toml",
            env = "SQLLOG2DB_CONFIG"
        )]
        config: String,
        /// Override config values before displaying
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
    },
    /// Count records in log files without exporting
    Stats {
        /// Configuration file path
        #[arg(
            short = 'c',
            long = "config",
            default_value = "config.toml",
            env = "SQLLOG2DB_CONFIG"
        )]
        config: String,
        /// Override config values, e.g. --set sqllog.directory=./logs
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
    },
    /// Generate shell completion scripts
    Completions {
        /// Shell type to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Self-update the application to the latest version
    SelfUpdate {
        /// Check for updates without performing the update
        #[arg(short = 'k', long = "check")]
        check: bool,
    },
    /// Print the man page to stdout
    Man,
}

impl Cli {
    /// Generate shell completions
    pub fn generate_completions(shell: Shell) {
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    }
}

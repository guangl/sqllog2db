use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};

/// SQL log exporter tool for DM database
#[derive(Debug, Parser)]
#[command(
    name = "sqllog2db",
    version,
    about = "Parse DM database SQL logs and export to CSV/SQLite",
    long_about = "A lightweight and efficient CLI tool for parsing DM database SQL logs (streaming) and exporting to CSV or SQLite."
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

    /// Output language: zh | en (default: auto-detect from LANG env var)
    #[arg(
        long = "lang",
        value_name = "LANG",
        global = true,
        env = "SQLLOG2DB_LANG"
    )]
    pub lang: Option<String>,

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
        /// Write CSV output to this file (shorthand for --set exporter.csv.file=<FILE>)
        #[arg(short = 'o', long = "output", value_name = "FILE")]
        output: Option<String>,
        /// Progress bar refresh interval in milliseconds
        #[arg(long = "progress-interval", default_value = "80", value_name = "MS")]
        progress_interval: u64,
        /// Skip files already processed in a previous run (tracked by state file)
        #[arg(long = "resume")]
        resume: bool,
        /// Override the state file path used by --resume (default: `.sqllog2db_state.toml`)
        #[arg(long = "state-file", value_name = "PATH", requires = "resume")]
        state_file: Option<String>,
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
        /// Override config values, e.g. --set sqllog.path=./logs
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
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
        /// Highlight fields that differ from the default configuration
        #[arg(long = "diff")]
        diff: bool,
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
        /// Override config values, e.g. --set sqllog.path=./logs
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Keep only records at or after this timestamp
        #[arg(long = "from", value_name = "DATETIME")]
        from: Option<String>,
        /// Keep only records at or before this timestamp
        #[arg(long = "to", value_name = "DATETIME")]
        to: Option<String>,
        /// Show top N slowest queries ranked by execution time
        #[arg(long = "top", value_name = "N")]
        top: Option<usize>,
        /// Output statistics as JSON (goes to stdout)
        #[arg(long = "json")]
        json: bool,
        /// Aggregate records by field(s): user, app, ip (repeatable, or comma-separated)
        #[arg(long = "group-by", value_name = "FIELD", value_delimiter = ',')]
        group_by: Vec<String>,
        /// Aggregate records into time buckets: hour, minute
        #[arg(long = "bucket", value_name = "GRANULARITY")]
        bucket: Option<String>,
    },
    /// Fingerprint SQL queries and aggregate by structure
    Digest {
        /// Configuration file path
        #[arg(
            short = 'c',
            long = "config",
            default_value = "config.toml",
            env = "SQLLOG2DB_CONFIG"
        )]
        config: String,
        /// Override config values, e.g. --set sqllog.path=./logs
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Keep only records at or after this timestamp
        #[arg(long = "from", value_name = "DATETIME")]
        from: Option<String>,
        /// Keep only records at or before this timestamp
        #[arg(long = "to", value_name = "DATETIME")]
        to: Option<String>,
        /// Show only top N fingerprints
        #[arg(long = "top", value_name = "N")]
        top: Option<usize>,
        /// Sort results by: count (default) or exec (total execution time)
        #[arg(long = "sort", value_name = "FIELD", default_value = "count")]
        sort: String,
        /// Skip fingerprints with fewer than N occurrences
        #[arg(long = "min-count", value_name = "N", default_value = "1")]
        min_count: u64,
        /// Output results as JSON (goes to stdout)
        #[arg(long = "json")]
        json: bool,
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

use crate::constants::LOG_LEVELS;
use crate::error::{ConfigError, Error, Result};
pub use crate::features::FeaturesConfig;
#[cfg(feature = "filters")]
#[allow(unused_imports)]
pub use crate::features::FiltersFeature;
use serde::Deserialize;
use std::path::Path;

#[cfg_attr(feature = "csv", derive(Default))]
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub sqllog: SqllogConfig,
    #[serde(default)]
    pub error: ErrorConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    #[cfg_attr(
        not(any(feature = "filters", feature = "replace_parameters")),
        allow(dead_code)
    )]
    pub features: FeaturesConfig,
    #[serde(default)]
    pub exporter: ExporterConfig,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|_| Error::Config(ConfigError::NotFound(path.to_path_buf())))?;
        toml::from_str(&content).map_err(|e| {
            Error::Config(ConfigError::ParseFailed {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })
        })
    }

    pub fn validate(&self) -> Result<()> {
        self.logging.validate()?;
        self.exporter.validate()?;
        self.sqllog.validate()?;
        FeaturesConfig::validate();
        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SqllogConfig {
    pub directory: String,
}

impl Default for SqllogConfig {
    fn default() -> Self {
        Self {
            directory: "sqllogs".to_string(),
        }
    }
}

impl SqllogConfig {
    pub fn validate(&self) -> Result<()> {
        if self.directory.trim().is_empty() {
            return Err(Error::Config(ConfigError::InvalidValue {
                field: "sqllog.directory".to_string(),
                value: self.directory.clone(),
                reason: "Input directory cannot be empty".to_string(),
            }));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ErrorConfig {
    #[serde(default = "default_error_file")]
    pub file: String,
}

fn default_error_file() -> String {
    "export/errors.log".to_string()
}

impl Default for ErrorConfig {
    fn default() -> Self {
        Self {
            file: "export/errors.log".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_logging_file")]
    pub file: String,
    #[serde(default = "default_logging_level")]
    pub level: String,
    #[serde(default = "default_retention_days")]
    pub retention_days: usize,
}

fn default_logging_file() -> String {
    "logs/sqllog2db.log".to_string()
}
fn default_logging_level() -> String {
    "info".to_string()
}
fn default_retention_days() -> usize {
    7
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            file: "logs/sqllog2db.log".to_string(),
            level: "info".to_string(),
            retention_days: 7,
        }
    }
}

impl LoggingConfig {
    pub fn validate(&self) -> Result<()> {
        if !LOG_LEVELS
            .iter()
            .any(|&l| l.eq_ignore_ascii_case(&self.level))
        {
            return Err(Error::Config(ConfigError::InvalidLogLevel {
                level: self.level.clone(),
                valid_levels: LOG_LEVELS.iter().map(|s| (*s).to_string()).collect(),
            }));
        }
        if self.retention_days == 0 || self.retention_days > 365 {
            return Err(Error::Config(ConfigError::InvalidValue {
                field: "logging.retention_days".to_string(),
                value: self.retention_days.to_string(),
                reason: "Retention days must be between 1 and 365".to_string(),
            }));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExporterConfig {
    #[cfg(feature = "csv")]
    pub csv: Option<CsvExporter>,
    #[cfg(feature = "jsonl")]
    pub jsonl: Option<JsonlExporter>,
    #[cfg(feature = "sqlite")]
    pub sqlite: Option<SqliteExporter>,
}

impl ExporterConfig {
    fn has_any(&self) -> bool {
        #[cfg(feature = "csv")]
        if self.csv.is_some() {
            return true;
        }
        #[cfg(feature = "jsonl")]
        if self.jsonl.is_some() {
            return true;
        }
        #[cfg(feature = "sqlite")]
        if self.sqlite.is_some() {
            return true;
        }
        false
    }

    pub fn validate(&self) -> Result<()> {
        if !self.has_any() {
            return Err(Error::Config(ConfigError::NoExporters));
        }
        Ok(())
    }
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            #[cfg(feature = "csv")]
            csv: Some(CsvExporter::default()),
            #[cfg(feature = "jsonl")]
            jsonl: None,
            #[cfg(feature = "sqlite")]
            sqlite: None,
        }
    }
}

#[cfg(feature = "csv")]
#[derive(Debug, Deserialize, Clone)]
pub struct CsvExporter {
    pub file: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    #[serde(default)]
    pub append: bool,
}

#[cfg(feature = "csv")]
impl Default for CsvExporter {
    fn default() -> Self {
        Self {
            file: "outputs/sqllog.csv".to_string(),
            overwrite: true,
            append: false,
        }
    }
}

#[cfg(feature = "jsonl")]
#[derive(Debug, Deserialize, Clone)]
pub struct JsonlExporter {
    pub file: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    #[serde(default)]
    pub append: bool,
}

#[cfg(feature = "jsonl")]
impl Default for JsonlExporter {
    fn default() -> Self {
        Self {
            file: "export/sqllog2db.jsonl".to_string(),
            overwrite: true,
            append: false,
        }
    }
}

#[cfg(feature = "sqlite")]
#[derive(Debug, Deserialize, Clone)]
pub struct SqliteExporter {
    pub database_url: String,
    #[serde(default = "default_table_name")]
    pub table_name: String,
    #[serde(default = "default_true")]
    pub overwrite: bool,
    #[serde(default)]
    pub append: bool,
}

#[cfg(feature = "sqlite")]
fn default_table_name() -> String {
    "sqllog_records".to_string()
}

#[cfg(feature = "sqlite")]
impl Default for SqliteExporter {
    fn default() -> Self {
        Self {
            database_url: "export/sqllog2db.db".to_string(),
            table_name: "sqllog_records".to_string(),
            overwrite: true,
            append: false,
        }
    }
}

fn default_true() -> bool {
    true
}

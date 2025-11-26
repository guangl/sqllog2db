#[cfg(feature = "parquet")]
#[derive(Debug, Deserialize)]
pub struct ParquetExporter {
    /// Parquet 输出文件路径
    pub file: String,
    /// 是否覆盖已存在的文件
    pub overwrite: bool,
    /// 每个 row group 的行数
    pub row_group_size: Option<usize>,
    /// 是否启用字典编码
    pub use_dictionary: Option<bool>,
}

#[cfg(feature = "parquet")]
impl Default for ParquetExporter {
    fn default() -> Self {
        Self {
            file: "export/sqllog2db.parquet".to_string(),
            overwrite: true,
            row_group_size: Some(100000),
            use_dictionary: Some(true),
        }
    }
}
use crate::constants::LOG_LEVELS;
use crate::error::{ConfigError, Error, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// 默认批量大小
fn default_batch_size() -> usize {
    10000
}

#[derive(Debug, Deserialize)]
pub struct Config {
    /// 新增：SQL 日志输入相关配置
    #[serde(default)]
    pub sqllog: SqllogConfig,

    pub error: ErrorConfig,
    pub logging: LoggingConfig,
    pub features: FeaturesConfig,
    pub exporter: ExporterConfig,
}

impl Config {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|_| Error::Config(ConfigError::NotFound(path.to_path_buf())))?;
        Self::from_str(&content, path.to_path_buf())
    }

    /// 从字符串解析配置
    pub fn from_str(content: &str, path: PathBuf) -> Result<Self> {
        let config: Config = toml::from_str(content).map_err(|e| {
            Error::Config(ConfigError::ParseFailed {
                path,
                reason: e.to_string(),
            })
        })?;

        // 验证配置
        config.validate()?;

        Ok(config)
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<()> {
        // 验证日志级别
        self.logging.validate()?;

        // 验证导出器配置
        self.exporter.validate()?;

        // 验证 sqllog 配置
        self.sqllog.validate()?;

        Ok(())
    }
}

/// SQL 日志输入配置
#[derive(Debug, Deserialize, Clone)]
pub struct SqllogConfig {
    /// SQL 日志输入目录（可包含多个日志文件）
    pub directory: String,
    /// 批量提交大小，0 表示全部解析完之后一次性写入，>0 表示每 N 条记录批量提交一次
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

impl Default for SqllogConfig {
    fn default() -> Self {
        Self {
            directory: "sqllog".to_string(),
            batch_size: 10000, // 默认使用 10000 批量大小以获得最佳性能
        }
    }
}

impl SqllogConfig {
    /// 获取 SQL 日志输入目录
    pub fn directory(&self) -> &str {
        &self.directory
    }

    /// 获取批量提交大小
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// 验证配置
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

#[derive(Debug, Deserialize)]
pub struct ErrorConfig {
    /// 错误日志输出文件路径
    pub file: String,
}

impl ErrorConfig {
    /// 获取错误日志输出文件路径
    pub fn file(&self) -> &str {
        &self.file
    }
}

impl Default for ErrorConfig {
    fn default() -> Self {
        Self {
            file: "errors.json".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    /// 应用日志输出文件路径
    pub file: String,
    pub level: String,
    #[serde(default = "default_retention_days")]
    pub retention_days: usize,
}

fn default_retention_days() -> usize {
    7
}

impl LoggingConfig {
    /// 获取日志输出文件路径
    pub fn file(&self) -> &str {
        &self.file
    }

    /// 获取日志级别
    pub fn level(&self) -> &str {
        &self.level
    }

    /// 获取日志保留天数
    pub fn retention_days(&self) -> usize {
        self.retention_days
    }

    /// 验证日志级别是否有效
    pub fn validate(&self) -> Result<()> {
        if !LOG_LEVELS
            .iter()
            .any(|&l| l.eq_ignore_ascii_case(self.level.as_str()))
        {
            return Err(Error::Config(ConfigError::InvalidLogLevel {
                level: self.level.clone(),
                valid_levels: LOG_LEVELS.iter().map(|s| s.to_string()).collect(),
            }));
        }

        // 验证保留天数（1-365天）
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

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            file: "logs/sqllog2db.log".to_string(),
            level: "info".to_string(),
            retention_days: 7,
        }
    }
}

/// 通用的 feature 开关
#[derive(Debug, Deserialize, Clone)]
pub struct ReplaceParametersFeature {
    pub enable: bool,
    pub symbols: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeaturesConfig {
    /// 对应配置文件中的 `[features.replace_parameters]`
    #[serde(default)]
    pub replace_parameters: Option<ReplaceParametersFeature>,
}

impl FeaturesConfig {
    /// 是否启用 SQL 参数替换
    pub fn should_replace_sql_parameters(&self) -> bool {
        self.replace_parameters
            .as_ref()
            .map(|f| f.enable)
            .unwrap_or(false)
    }
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            replace_parameters: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ExporterConfig {
    #[cfg(feature = "csv")]
    pub csv: Option<CsvExporter>,
    #[cfg(feature = "parquet")]
    pub parquet: Option<ParquetExporter>,
}

#[cfg(feature = "parquet")]
#[derive(Debug, Deserialize)]
pub struct ParquetExporter {
    /// Parquet 输出文件路径
    pub file: String,
    /// 是否覆盖已存在的文件
    pub overwrite: bool,
    /// 每个 row group 的行数
    pub row_group_size: Option<usize>,
    /// 是否启用字典编码
    pub use_dictionary: Option<bool>,
}

#[cfg(feature = "parquet")]
impl Default for ParquetExporter {
    fn default() -> Self {
        Self {
            file: "export/sqllog2db.parquet".to_string(),
            overwrite: true,
            row_group_size: Some(100000),
            use_dictionary: Some(true),
        }
    }
}

impl ExporterConfig {
    /// 获取 CSV 导出器配置
    pub fn csv(&self) -> Option<&CsvExporter> {
        #[cfg(feature = "csv")]
        {
            return self.csv.as_ref();
        }
        #[cfg(not(feature = "csv"))]
        {
            return None;
        }
    }

    #[cfg(feature = "parquet")]
    /// 获取 Parquet 导出器配置
    pub fn parquet(&self) -> Option<&ParquetExporter> {
        return self.parquet.as_ref();
    }

    /// 检查是否有任何导出器配置
    pub fn has_exporters(&self) -> bool {
        let mut found = false;
        #[cfg(feature = "csv")]
        {
            found = found || self.csv.is_some();
        }
        #[cfg(feature = "parquet")]
        {
            found = found || self.parquet.is_some();
        }
        found
    }

    /// 统计配置的导出器总数
    pub fn total_exporters(&self) -> usize {
        let mut count = 0;
        #[cfg(feature = "csv")]
        {
            if self.csv.is_some() {
                count += 1;
            }
        }
        #[cfg(feature = "parquet")]
        {
            if self.parquet.is_some() {
                count += 1;
            }
        }
        count
    }

    /// 验证导出器配置（只支持单个导出器）
    pub fn validate(&self) -> Result<()> {
        if !self.has_exporters() {
            return Err(Error::Config(ConfigError::NoExporters));
        }

        let total = self.total_exporters();
        if total > 1 {
            eprintln!(
                "Warning: {} exporters configured, but only one is supported.",
                total
            );
            eprintln!("Will use the first exporter by priority: CSV > Parquet > SQLite");
        }

        Ok(())
    }
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            #[cfg(feature = "csv")]
            csv: Some(CsvExporter::default()),
            #[cfg(feature = "parquet")]
            parquet: None,
        }
    }
}

#[cfg(feature = "parquet")]
#[derive(Debug, Deserialize)]
pub struct ParquetExporter {
    /// Parquet 输出文件路径
    pub file: String,
    /// 是否覆盖已存在的文件
    pub overwrite: bool,
    /// 每个 row group 的行数
    pub row_group_size: Option<usize>,
    /// 是否启用字典编码
    pub use_dictionary: Option<bool>,
}

#[cfg(feature = "parquet")]
impl Default for ParquetExporter {
    fn default() -> Self {
        Self {
            file: "export/sqllog2db.parquet".to_string(),
            overwrite: true,
            row_group_size: Some(100000),
            use_dictionary: Some(true),
        }
    }
}

#[cfg(feature = "csv")]
#[derive(Debug, Deserialize)]
pub struct CsvExporter {
    /// CSV 输出文件路径
    pub file: String,
    /// 是否覆盖已存在的文件
    pub overwrite: bool,
    /// 是否追加模式（暂未实现）
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

#[cfg(feature = "csv")]
impl Default for Config {
    fn default() -> Self {
        Self {
            sqllog: SqllogConfig::default(),
            error: ErrorConfig::default(),
            logging: LoggingConfig::default(),
            features: FeaturesConfig::default(),
            exporter: ExporterConfig::default(),
        }
    }
}

use crate::constants::LOG_LEVELS;
use crate::error::{ConfigError, Error, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// 默认表名
#[cfg(any(
    feature = "sqlite",
    feature = "duckdb",
    feature = "postgres",
    feature = "dm"
))]
fn default_table_name() -> String {
    "sqllog_records".to_string()
}

/// 默认 true 值
#[cfg(any(
    feature = "sqlite",
    feature = "duckdb",
    feature = "postgres",
    feature = "dm"
))]
fn default_true() -> bool {
    true
}

/// `PostgreSQL` 默认主机
#[cfg(feature = "postgres")]
fn default_postgres_host() -> String {
    "localhost".to_string()
}

/// `PostgreSQL` 默认端口
#[cfg(feature = "postgres")]
fn default_postgres_port() -> u16 {
    5432
}

/// `PostgreSQL` 默认用户名
#[cfg(feature = "postgres")]
fn default_postgres_username() -> String {
    "postgres".to_string()
}

/// `PostgreSQL` 默认数据库
#[cfg(feature = "postgres")]
fn default_postgres_database() -> String {
    "sqllog".to_string()
}

/// `PostgreSQL` 默认 schema
#[cfg(feature = "postgres")]
fn default_postgres_schema() -> String {
    "public".to_string()
}

#[cfg_attr(feature = "csv", derive(Default))]
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// 新增：SQL 日志输入相关配置
    #[serde(default)]
    pub sqllog: SqllogConfig,
    #[serde(default)]
    pub error: ErrorConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub features: FeaturesConfig,
    #[serde(default)]
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

        // 验证功能配置
        FeaturesConfig::validate();

        Ok(())
    }
}

/// SQL 日志输入配置
#[derive(Debug, Deserialize, Clone)]
pub struct SqllogConfig {
    /// SQL 日志输入目录（可包含多个日志文件）
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
    /// 获取 SQL 日志输入目录
    #[must_use]
    pub fn directory(&self) -> &str {
        &self.directory
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

/// 过滤器配置
#[derive(Debug, Deserialize, Clone, Default)]
pub struct FiltersFeature {
    /// 是否启用过滤器
    pub enable: bool,
    /// 过滤事务 ID 列表
    pub trxids: Option<Vec<String>>,
    /// 过滤执行 ID 列表 (关联事务过滤)
    pub exec_ids: Option<Vec<i64>>,
    /// 过滤客户端 IP 列表
    pub client_ips: Option<Vec<String>>,
    /// 过滤会话 ID 列表
    pub sess_ids: Option<Vec<String>>,
    /// 过滤线程 ID 列表
    pub thrd_ids: Option<Vec<String>>,
    /// 过滤用户名列表
    pub usernames: Option<Vec<String>>,
    /// 过滤语句类型列表 (如 INS, UPD, SEL)
    pub statements: Option<Vec<String>>,
    /// 过滤应用名称列表
    pub appnames: Option<Vec<String>>,
}

impl FiltersFeature {
    /// 验证过滤器配置
    pub fn validate() {}

    /// 检查是否提供了执行 ID 过滤
    #[must_use]
    pub fn has_exec_id_filters(&self) -> bool {
        self.enable && self.exec_ids.as_ref().is_some_and(|ids| !ids.is_empty())
    }

    /// 检查是否应该根据元数据保留该记录 (满足任意一个配置的过滤器即可 - OR 逻辑)
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn should_keep_meta(
        &self,
        trxid: &str,
        ip: &str,
        sess: &str,
        thrd: &str,
        user: &str,
        stmt: &str,
        app: &str,
    ) -> bool {
        if !self.enable {
            return true;
        }

        let has_trxid = self.trxids.as_ref().is_some_and(|v| !v.is_empty());
        let has_ip = self.client_ips.as_ref().is_some_and(|v| !v.is_empty());
        let has_sess = self.sess_ids.as_ref().is_some_and(|v| !v.is_empty());
        let has_thrd = self.thrd_ids.as_ref().is_some_and(|v| !v.is_empty());
        let has_user = self.usernames.as_ref().is_some_and(|v| !v.is_empty());
        let has_stmt = self.statements.as_ref().is_some_and(|v| !v.is_empty());
        let has_app = self.appnames.as_ref().is_some_and(|v| !v.is_empty());

        // 如果没有配置任何过滤器，则保留所有记录
        if !has_trxid && !has_ip && !has_sess && !has_thrd && !has_user && !has_stmt && !has_app {
            return true;
        }

        // OR 逻辑：只要命中一个过滤器就保留
        (has_trxid && self.should_keep_trxid(trxid))
            || (has_ip && self.should_keep_ip(ip))
            || (has_sess && self.should_keep_sess(sess))
            || (has_thrd && self.should_keep_thrd(thrd))
            || (has_user && self.should_keep_user(user))
            || (has_stmt && self.should_keep_stmt(stmt))
            || (has_app && self.should_keep_app(app))
    }

    /// 检查是否应该保留该事务 ID
    #[must_use]
    pub fn should_keep_trxid(&self, trxid: &str) -> bool {
        if let Some(trxids) = &self.trxids {
            if trxids.is_empty() {
                return true;
            }
            return trxids.iter().any(|id| id == trxid);
        }
        true
    }

    /// 检查是否应该保留该 IP
    #[must_use]
    pub fn should_keep_ip(&self, ip: &str) -> bool {
        if let Some(ips) = &self.client_ips {
            if ips.is_empty() {
                return true;
            }
            return ips.iter().any(|filter_ip| ip.contains(filter_ip));
        }
        true
    }

    /// 检查是否应该保留该会话 ID
    #[must_use]
    pub fn should_keep_sess(&self, sess: &str) -> bool {
        if let Some(ids) = &self.sess_ids {
            if ids.is_empty() {
                return true;
            }
            return ids.iter().any(|id| sess.contains(id));
        }
        true
    }

    /// 检查是否应该保留该线程 ID
    #[must_use]
    pub fn should_keep_thrd(&self, thrd: &str) -> bool {
        if let Some(ids) = &self.thrd_ids {
            if ids.is_empty() {
                return true;
            }
            return ids.iter().any(|id| thrd.contains(id));
        }
        true
    }

    /// 检查是否应该保留该用户名
    #[must_use]
    pub fn should_keep_user(&self, user: &str) -> bool {
        if let Some(users) = &self.usernames {
            if users.is_empty() {
                return true;
            }
            return users.iter().any(|u| user.contains(u));
        }
        true
    }

    /// 检查是否应该保留该语句类型
    #[must_use]
    pub fn should_keep_stmt(&self, stmt: &str) -> bool {
        if let Some(stmts) = &self.statements {
            if stmts.is_empty() {
                return true;
            }
            return stmts.iter().any(|s| stmt.contains(s));
        }
        true
    }

    /// 检查是否应该保留该应用名称
    #[must_use]
    pub fn should_keep_app(&self, app: &str) -> bool {
        if let Some(apps) = &self.appnames {
            if apps.is_empty() {
                return true;
            }
            return apps.iter().any(|a| app.contains(a));
        }
        true
    }

    /// 合并额外的事务 ID
    pub fn merge_trxids(&mut self, extra_trxids: Vec<String>) {
        if extra_trxids.is_empty() {
            return;
        }
        let mut current = self.trxids.take().unwrap_or_default();
        for id in extra_trxids {
            if !current.contains(&id) {
                current.push(id);
            }
        }
        self.trxids = Some(current);
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ErrorConfig {
    /// 错误日志输出文件路径
    pub file: String,
}

impl ErrorConfig {
    /// 获取错误日志输出文件路径
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }
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
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// 获取日志级别
    #[must_use]
    pub fn level(&self) -> &str {
        &self.level
    }

    /// 获取日志保留天数
    #[must_use]
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
                valid_levels: LOG_LEVELS.iter().map(|s| (*s).to_string()).collect(),
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

#[derive(Debug, Deserialize, Clone, Default)]
pub struct FeaturesConfig {
    /// 对应配置文件中的 `[features.replace_parameters]`
    #[serde(default)]
    pub replace_parameters: Option<ReplaceParametersFeature>,
    /// 对应配置文件中的 `[features.filters]`
    #[serde(default)]
    pub filters: Option<FiltersFeature>,
}

impl FeaturesConfig {
    /// 是否启用 SQL 参数替换
    #[must_use]
    pub fn should_replace_sql_parameters(&self) -> bool {
        self.replace_parameters.as_ref().is_some_and(|f| f.enable)
    }

    /// 验证配置
    pub fn validate() {
        FiltersFeature::validate();
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExporterConfig {
    #[cfg(feature = "csv")]
    pub csv: Option<CsvExporter>,
    #[cfg(feature = "parquet")]
    pub parquet: Option<ParquetExporter>,
    #[cfg(feature = "jsonl")]
    pub jsonl: Option<JsonlExporter>,
    #[cfg(feature = "sqlite")]
    pub sqlite: Option<SqliteExporter>,
    #[cfg(feature = "duckdb")]
    pub duckdb: Option<DuckdbExporter>,
    #[cfg(feature = "postgres")]
    pub postgres: Option<PostgresExporter>,
    #[cfg(feature = "dm")]
    pub dm: Option<DmExporter>,
}

impl ExporterConfig {
    /// 获取 CSV 导出器配置
    #[cfg(feature = "csv")]
    #[must_use]
    pub fn csv(&self) -> Option<&CsvExporter> {
        self.csv.as_ref()
    }

    #[cfg(feature = "parquet")]
    /// 获取 Parquet 导出器配置
    #[must_use]
    pub fn parquet(&self) -> Option<&ParquetExporter> {
        self.parquet.as_ref()
    }

    #[cfg(feature = "jsonl")]
    /// 获取 JSONL 导出器配置
    #[must_use]
    pub fn jsonl(&self) -> Option<&JsonlExporter> {
        self.jsonl.as_ref()
    }

    #[cfg(feature = "sqlite")]
    /// 获取 `SQLite` 导出器配置
    #[must_use]
    pub fn sqlite(&self) -> Option<&SqliteExporter> {
        self.sqlite.as_ref()
    }

    #[cfg(feature = "duckdb")]
    /// 获取 `DuckDB` 导出器配置
    #[must_use]
    pub fn duckdb(&self) -> Option<&DuckdbExporter> {
        self.duckdb.as_ref()
    }

    #[cfg(feature = "postgres")]
    /// 获取 `PostgreSQL` 导出器配置
    #[must_use]
    pub fn postgres(&self) -> Option<&PostgresExporter> {
        self.postgres.as_ref()
    }

    #[cfg(feature = "dm")]
    /// 获取 DM 导出器配置
    #[must_use]
    pub fn dm(&self) -> Option<&DmExporter> {
        self.dm.as_ref()
    }

    /// 检查是否有任何导出器配置
    #[must_use]
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
        #[cfg(feature = "jsonl")]
        {
            found = found || self.jsonl.is_some();
        }
        #[cfg(feature = "sqlite")]
        {
            found = found || self.sqlite.is_some();
        }
        #[cfg(feature = "duckdb")]
        {
            found = found || self.duckdb.is_some();
        }
        #[cfg(feature = "postgres")]
        {
            found = found || self.postgres.is_some();
        }
        #[cfg(feature = "dm")]
        {
            found = found || self.dm.is_some();
        }
        found
    }

    /// 统计配置的导出器总数
    #[must_use]
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
        #[cfg(feature = "jsonl")]
        {
            if self.jsonl.is_some() {
                count += 1;
            }
        }
        #[cfg(feature = "sqlite")]
        {
            if self.sqlite.is_some() {
                count += 1;
            }
        }
        #[cfg(feature = "duckdb")]
        {
            if self.duckdb.is_some() {
                count += 1;
            }
        }
        #[cfg(feature = "postgres")]
        {
            if self.postgres.is_some() {
                count += 1;
            }
        }
        #[cfg(feature = "dm")]
        {
            if self.dm.is_some() {
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
            eprintln!("Warning: {total} exporters configured, but only one is supported.");
            eprintln!(
                "Will use the first exporter by priority: CSV > Parquet > JSONL > SQLite > DuckDB > PostgreSQL > DM"
            );
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
            parquet: Some(ParquetExporter::default()),
            #[cfg(feature = "jsonl")]
            jsonl: None,
            #[cfg(feature = "sqlite")]
            sqlite: None,
            #[cfg(feature = "duckdb")]
            duckdb: None,
            #[cfg(feature = "postgres")]
            postgres: None,
            #[cfg(feature = "dm")]
            dm: None,
        }
    }
}

#[cfg(feature = "parquet")]
#[derive(Debug, Deserialize, Clone)]
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
            row_group_size: Some(100_000),
            use_dictionary: Some(true),
        }
    }
}

#[cfg(feature = "csv")]
#[derive(Debug, Deserialize, Clone)]
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

#[cfg(feature = "jsonl")]
#[derive(Debug, Deserialize, Clone)]
pub struct JsonlExporter {
    /// JSONL 输出文件路径
    pub file: String,
    /// 是否覆盖已存在的文件
    pub overwrite: bool,
    /// 是否追加模式
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
    /// `SQLite` 数据库文件路径
    pub database_url: String,
    /// 表名
    #[serde(default = "default_table_name")]
    pub table_name: String,
    /// 是否覆盖已存在的表
    #[serde(default = "default_true")]
    pub overwrite: bool,
    /// 是否追加模式
    #[serde(default)]
    pub append: bool,
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

#[cfg(feature = "duckdb")]
#[derive(Debug, Deserialize, Clone)]
pub struct DuckdbExporter {
    /// `DuckDB` 数据库文件路径
    pub database_url: String,
    /// 表名
    #[serde(default = "default_table_name")]
    pub table_name: String,
    /// 是否覆盖已存在的表
    #[serde(default = "default_true")]
    pub overwrite: bool,
    /// 是否追加模式
    #[serde(default)]
    pub append: bool,
}

#[cfg(feature = "duckdb")]
impl Default for DuckdbExporter {
    fn default() -> Self {
        Self {
            database_url: "export/sqllog2db.duckdb".to_string(),
            table_name: "sqllog_records".to_string(),
            overwrite: true,
            append: false,
        }
    }
}

#[cfg(feature = "postgres")]
#[derive(Debug, Deserialize, Clone)]
pub struct PostgresExporter {
    /// `PostgreSQL` 主机地址
    #[serde(default = "default_postgres_host")]
    pub host: String,
    /// `PostgreSQL` 端口
    #[serde(default = "default_postgres_port")]
    pub port: u16,
    /// 用户名
    #[serde(default = "default_postgres_username")]
    pub username: String,
    /// 密码
    pub password: String,
    /// 数据库名
    #[serde(default = "default_postgres_database")]
    pub database: String,
    /// Schema 名称
    #[serde(default = "default_postgres_schema")]
    pub schema: String,
    /// 表名
    #[serde(default = "default_table_name")]
    pub table_name: String,
    /// 是否覆盖已存在的表
    #[serde(default = "default_true")]
    pub overwrite: bool,
    /// 是否追加模式
    #[serde(default)]
    pub append: bool,
}

#[cfg(feature = "postgres")]
impl Default for PostgresExporter {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            username: "postgres".to_string(),
            password: "postgres".to_string(),
            database: "sqllog".to_string(),
            schema: "public".to_string(),
            table_name: "sqllog_records".to_string(),
            overwrite: true,
            append: false,
        }
    }
}

#[cfg(feature = "postgres")]
impl PostgresExporter {
    /// 获取连接字符串
    #[must_use]
    pub fn connection_string(&self) -> String {
        if self.password.is_empty() {
            format!(
                "host={} port={} user={} dbname={}",
                self.host, self.port, self.username, self.database
            )
        } else {
            format!(
                "host={} port={} user={} password={} dbname={}",
                self.host, self.port, self.username, self.password, self.database
            )
        }
    }
}

#[cfg(feature = "dm")]
#[derive(Debug, Deserialize, Clone)]
pub struct DmExporter {
    /// DM 数据库连接字符串 (例如: SYSDBA/SYSDBA@localhost:5236)
    pub userid: String,
    /// 表名
    #[serde(default = "default_table_name")]
    pub table_name: String,
    /// 控制文件路径
    pub control_file: String,
    /// 日志目录
    pub log_dir: String,
}

#[cfg(feature = "dm")]
impl Default for DmExporter {
    fn default() -> Self {
        Self {
            userid: "SYSDBA/SYSDBA@localhost:5236".to_string(),
            table_name: "sqllog_records".to_string(),
            control_file: "export/sqllog.ctl".to_string(),
            log_dir: "export/log".to_string(),
        }
    }
}

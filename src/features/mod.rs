#[cfg(feature = "filters")]
pub mod filters;
#[cfg(feature = "filters")]
#[allow(unused_imports)]
pub use filters::{FiltersFeature, IndicatorFilters, MetaFilters, SqlFilters};

#[cfg(feature = "replace_parameters")]
pub mod replace_parameters;
#[cfg(feature = "replace_parameters")]
#[allow(unused_imports)]
pub use replace_parameters::{ParamValue, apply_params, compute_normalized, parse_params};

use dm_database_parser_sqllog::Sqllog;
use serde::Deserialize;

/// `[features.replace_parameters]` 配置段
#[cfg(feature = "replace_parameters")]
#[derive(Debug, Deserialize, Clone)]
pub struct ReplaceParametersConfig {
    /// 是否在导出结果中写入 `normalized_sql` 列（默认 true）
    #[serde(default = "default_true")]
    pub enable: bool,
}

#[cfg(feature = "replace_parameters")]
impl Default for ReplaceParametersConfig {
    fn default() -> Self {
        Self { enable: true }
    }
}

#[cfg(feature = "replace_parameters")]
fn default_true() -> bool {
    true
}

/// 功能开关配置
#[derive(Debug, Deserialize, Clone, Default)]
pub struct FeaturesConfig {
    #[cfg(feature = "filters")]
    pub filters: Option<FiltersFeature>,
    #[cfg(feature = "replace_parameters")]
    pub replace_parameters: Option<ReplaceParametersConfig>,
}

impl FeaturesConfig {
    pub fn validate() {
        #[cfg(feature = "filters")]
        FiltersFeature::validate();
    }
}

/// 记录处理器接口：实现此接口即可加入处理管线
/// 返回 true 表示保留该记录，false 表示丢弃
pub trait LogProcessor: Send + Sync + std::fmt::Debug {
    fn process(&self, record: &Sqllog) -> bool;
}

/// 处理管线：按顺序执行处理器，任一返回 false 则丢弃记录
#[derive(Debug, Default)]
pub struct Pipeline {
    processors: Vec<Box<dyn LogProcessor>>,
}

impl Pipeline {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加处理器到管线末尾
    #[cfg(feature = "filters")]
    pub fn add(&mut self, processor: Box<dyn LogProcessor>) {
        self.processors.push(processor);
    }

    /// 管线是否为空（空管线可以走零开销快速路径）
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.processors.is_empty()
    }

    /// 顺序执行所有处理器
    #[inline]
    #[must_use]
    pub fn run(&self, record: &Sqllog) -> bool {
        self.processors.iter().all(|p| p.process(record))
    }
}

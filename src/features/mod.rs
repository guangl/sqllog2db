pub mod filters;
pub mod replace_parameters;

pub use filters::*;
pub use replace_parameters::*;

use serde::Deserialize;

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

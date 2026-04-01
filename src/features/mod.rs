#[cfg(feature = "filters")]
pub mod filters;
#[cfg(feature = "replace_parameters")]
pub mod replace_parameters;

#[cfg(feature = "filters")]
pub use filters::*;
#[cfg(feature = "replace_parameters")]
pub use replace_parameters::*;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct FeaturesConfig {
    /// 对应配置文件中的 `[features.replace_parameters]`
    #[cfg(feature = "replace_parameters")]
    #[serde(default)]
    pub replace_parameters: Option<ReplaceParametersFeature>,
    /// 对应配置文件中的 `[features.filters]`
    #[cfg(feature = "filters")]
    #[serde(default)]
    pub filters: Option<FiltersFeature>,
}

impl FeaturesConfig {
    /// 是否启用 SQL 参数替换
    #[must_use]
    pub fn should_replace_sql_parameters(&self) -> bool {
        #[cfg(feature = "replace_parameters")]
        {
            self.replace_parameters.as_ref().is_some_and(|f| f.enable)
        }
        #[cfg(not(feature = "replace_parameters"))]
        {
            false
        }
    }

    /// 验证配置
    pub fn validate() {
        #[cfg(feature = "filters")]
        FiltersFeature::validate();
    }
}

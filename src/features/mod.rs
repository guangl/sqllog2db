#[cfg(feature = "filters")]
pub mod filters;

#[cfg(feature = "filters")]
pub use filters::*;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct FeaturesConfig {
    /// 对应配置文件中的 `[features.filters]`
    #[cfg(feature = "filters")]
    #[serde(default)]
    pub filters: Option<FiltersFeature>,
}

impl FeaturesConfig {
    /// 验证配置
    pub fn validate() {
        #[cfg(feature = "filters")]
        FiltersFeature::validate();
    }
}

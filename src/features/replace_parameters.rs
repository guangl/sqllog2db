use serde::Deserialize;

/// 通用的 feature 开关
#[derive(Debug, Deserialize, Clone)]
pub struct ReplaceParametersFeature {
    pub enable: bool,
    pub symbols: Option<Vec<String>>,
}

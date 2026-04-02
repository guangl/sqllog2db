#[cfg(feature = "filters")]
pub mod filters;
#[cfg(feature = "filters")]
#[allow(unused_imports)]
pub use filters::{FiltersFeature, IndicatorFilters, MetaFilters, SqlFilters};

use dm_database_parser_sqllog::Sqllog;
use serde::Deserialize;

/// 功能开关配置
#[derive(Debug, Deserialize, Clone, Default)]
pub struct FeaturesConfig {
    #[cfg(feature = "filters")]
    pub filters: Option<FiltersFeature>,
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

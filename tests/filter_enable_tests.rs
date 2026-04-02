#[cfg(all(test, feature = "filters"))]
mod tests {
    use dm_database_sqllog2db::features::filters::{FiltersFeature, IndicatorFilters, MetaFilters};

    #[test]
    fn test_has_transaction_filters_respects_enable() {
        let feature = FiltersFeature {
            enable: false,
            indicators: IndicatorFilters {
                min_runtime_ms: Some(100),
                ..Default::default()
            },
            ..Default::default()
        };

        // Current behavior (likely buggy): returns true if indicators have filters even if enable is false
        // Expected behavior: should return false if enable is false
        assert!(
            !feature.has_transaction_filters(),
            "Should be false when enable is false"
        );
    }

    #[test]
    fn test_should_keep_when_enabled_and_no_match() {
        let feature = FiltersFeature {
            enable: true,
            meta: MetaFilters {
                start_ts: Some("2023-01-01".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        // Should filter out if before start_ts
        assert!(!feature.should_keep("2022-01-01", "", "", "", "", "", "", "", None));
        // Should keep if after start_ts
        assert!(feature.should_keep("2023-01-01", "", "", "", "", "", "", "", None));
    }
}

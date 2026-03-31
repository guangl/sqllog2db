use dm_database_sqllog2db::features::FiltersFeature;

#[test]
fn test_filter_by_tag() {
    let mut filters = FiltersFeature::default();
    filters.enable = true;
    filters.meta.tags = Some(vec!["IMPORTANT".to_string()]);

    // Test with matching tag
    assert!(filters.should_keep(
        "2023-01-01 10:00:00",
        "trx1",
        "127.0.0.1",
        "sess1",
        "thrd1",
        "user1",
        "SELECT",
        "app1",
        Some("IMPORTANT")
    ));

    // Test with non-matching tag
    assert!(!filters.should_keep(
        "2023-01-01 10:00:00",
        "trx1",
        "127.0.0.1",
        "sess1",
        "thrd1",
        "user1",
        "SELECT",
        "app1",
        Some("NORMAL")
    ));

    // Test with None tag
    assert!(!filters.should_keep(
        "2023-01-01 10:00:00",
        "trx1",
        "127.0.0.1",
        "sess1",
        "thrd1",
        "user1",
        "SELECT",
        "app1",
        None
    ));
}

#[test]
fn test_filter_with_multiple_tags() {
    let mut filters = FiltersFeature::default();
    filters.enable = true;
    filters.meta.tags = Some(vec!["IMPORTANT".to_string(), "CRITICAL".to_string()]);

    assert!(filters.should_keep(
        "2023-01-01 10:00:00",
        "trx1",
        "127.0.0.1",
        "sess1",
        "thrd1",
        "user1",
        "SELECT",
        "app1",
        Some("IMPORTANT")
    ));

    assert!(filters.should_keep(
        "2023-01-01 10:00:00",
        "trx1",
        "127.0.0.1",
        "sess1",
        "thrd1",
        "user1",
        "SELECT",
        "app1",
        Some("CRITICAL")
    ));

    assert!(!filters.should_keep(
        "2023-01-01 10:00:00",
        "trx1",
        "127.0.0.1",
        "sess1",
        "thrd1",
        "user1",
        "SELECT",
        "app1",
        Some("DEBUG")
    ));
}

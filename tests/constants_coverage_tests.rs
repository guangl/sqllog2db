//! 针对 Constants 和其他辅助模块的覆盖测试
use dm_database_sqllog2db::constants::LOG_LEVELS;

#[test]
fn test_log_levels_contains_trace() {
    assert!(LOG_LEVELS.contains(&"trace"));
}

#[test]
fn test_log_levels_contains_debug() {
    assert!(LOG_LEVELS.contains(&"debug"));
}

#[test]
fn test_log_levels_contains_info() {
    assert!(LOG_LEVELS.contains(&"info"));
}

#[test]
fn test_log_levels_contains_warn() {
    assert!(LOG_LEVELS.contains(&"warn"));
}

#[test]
fn test_log_levels_contains_error() {
    assert!(LOG_LEVELS.contains(&"error"));
}

#[test]
fn test_log_levels_count() {
    assert_eq!(LOG_LEVELS.len(), 5);
}

#[test]
fn test_log_levels_order() {
    assert_eq!(LOG_LEVELS[0], "trace");
    assert_eq!(LOG_LEVELS[1], "debug");
    assert_eq!(LOG_LEVELS[2], "info");
    assert_eq!(LOG_LEVELS[3], "warn");
    assert_eq!(LOG_LEVELS[4], "error");
}

#[test]
fn test_log_levels_iteration() {
    let mut count = 0;
    for level in LOG_LEVELS {
        assert!(!level.is_empty());
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn test_log_levels_case_sensitive() {
    assert!(!LOG_LEVELS.contains(&"INFO"));
    assert!(!LOG_LEVELS.contains(&"Debug"));
    assert!(!LOG_LEVELS.contains(&"ERROR"));
}

#[test]
fn test_log_levels_invalid() {
    assert!(!LOG_LEVELS.contains(&"fatal"));
    assert!(!LOG_LEVELS.contains(&"critical"));
    assert!(!LOG_LEVELS.contains(&"notice"));
}

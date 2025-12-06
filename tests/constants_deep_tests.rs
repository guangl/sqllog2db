//! 为 constants.rs 的覆盖测试
#[cfg(test)]
mod constants_tests {
    use dm_database_sqllog2db::constants::*;

    #[test]
    fn test_log_levels_defined() {
        assert!(!LOG_LEVELS.is_empty());
        assert!(LOG_LEVELS.len() <= 10);
    }

    #[test]
    fn test_log_levels_contains_common_levels() {
        assert!(LOG_LEVELS.contains(&"trace"));
        assert!(LOG_LEVELS.contains(&"debug"));
        assert!(LOG_LEVELS.contains(&"info"));
        assert!(LOG_LEVELS.contains(&"warn"));
        assert!(LOG_LEVELS.contains(&"error"));
    }

    #[test]
    fn test_log_levels_are_lowercase() {
        for level in LOG_LEVELS {
            let lowercase = level.to_lowercase();
            assert_eq!(*level, lowercase);
        }
    }

    #[test]
    fn test_log_levels_iteration() {
        let mut count = 0;
        for _ in LOG_LEVELS {
            count += 1;
        }
        assert_eq!(count, LOG_LEVELS.len());
    }

    #[test]
    fn test_log_levels_valid_strings() {
        for level in LOG_LEVELS {
            assert!(!level.is_empty());
            assert!(level.len() < 20);
        }
    }

    #[test]
    fn test_log_levels_exact_set() {
        let expected = vec!["trace", "debug", "info", "warn", "error"];
        assert_eq!(LOG_LEVELS.to_vec(), expected);
    }

    #[test]
    fn test_log_levels_includes_error() {
        let has_error = LOG_LEVELS.contains(&"error");
        assert!(has_error);
    }

    #[test]
    fn test_log_levels_includes_info() {
        let has_info = LOG_LEVELS.contains(&"info");
        assert!(has_info);
    }

    #[test]
    fn test_log_levels_includes_debug() {
        let has_debug = LOG_LEVELS.contains(&"debug");
        assert!(has_debug);
    }

    #[test]
    fn test_log_levels_order() {
        // Should be ordered from least to most verbose
        assert!(LOG_LEVELS.len() >= 3);
        assert_eq!(LOG_LEVELS[0], "trace");
        assert_eq!(LOG_LEVELS[4], "error");
    }

    #[test]
    fn test_log_levels_with_filter() {
        let warn_and_above: Vec<_> = LOG_LEVELS
            .iter()
            .filter(|&&l| l == "warn" || l == "error")
            .collect();
        assert!(warn_and_above.len() >= 2);
    }

    #[test]
    fn test_log_levels_find() {
        let found = LOG_LEVELS.iter().find(|&&l| l == "info");
        assert!(found.is_some());
        assert_eq!(*found.unwrap(), "info");
    }

    #[test]
    fn test_log_levels_position() {
        let pos = LOG_LEVELS.iter().position(|&l| l == "debug");
        assert!(pos.is_some());
        assert_eq!(pos.unwrap(), 1);
    }

    #[test]
    fn test_log_levels_count() {
        let count = LOG_LEVELS.len();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_log_levels_all() {
        let all_valid = LOG_LEVELS.iter().all(|l| !l.is_empty());
        assert!(all_valid);
    }

    #[test]
    fn test_log_levels_any_trace() {
        let has_trace = LOG_LEVELS.contains(&"trace");
        assert!(has_trace);
    }
}

//! JSONL 导出器的完整测试
#![cfg(feature = "jsonl")]

use dm_database_sqllog2db::config;
use dm_database_sqllog2db::exporter::{Exporter, JsonlExporter};
use std::fs;
use std::path::PathBuf;

fn setup_test_dir(name: &str) -> PathBuf {
    let test_dir = PathBuf::from("target/test_jsonl_exporter").join(name);
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    test_dir
}

#[test]
fn test_jsonl_exporter_new() {
    let test_dir = setup_test_dir("new");
    let output_file = test_dir.join("output.jsonl");

    let exporter = JsonlExporter::new(&output_file, false);
    assert_eq!(exporter.name(), "JSONL");
}

#[test]
fn test_jsonl_exporter_initialize() {
    let test_dir = setup_test_dir("initialize");
    let output_file = test_dir.join("output.jsonl");

    let mut exporter = JsonlExporter::new(&output_file, false);
    let result = exporter.initialize();

    assert!(result.is_ok());
    assert!(output_file.exists());
}

#[test]
fn test_jsonl_exporter_from_config() {
    let test_dir = setup_test_dir("from_config");
    let output_file = test_dir.join("output.jsonl");

    let config = config::JsonlExporter {
        file: output_file.to_str().unwrap().to_string(),
        overwrite: true,
        append: false,
    };

    let mut exporter = JsonlExporter::from_config(&config);
    assert_eq!(exporter.name(), "JSONL");

    exporter.initialize().unwrap();
    assert!(output_file.exists());
}

#[test]
fn test_jsonl_exporter_from_config_append_mode() {
    let test_dir = setup_test_dir("append_mode");
    let output_file = test_dir.join("output.jsonl");

    let config = config::JsonlExporter {
        file: output_file.to_str().unwrap().to_string(),
        overwrite: false,
        append: true,
    };

    let mut exporter = JsonlExporter::from_config(&config);
    exporter.initialize().unwrap();
    assert!(output_file.exists());
}

#[test]
fn test_jsonl_exporter_stats() {
    let test_dir = setup_test_dir("stats");
    let output_file = test_dir.join("output.jsonl");

    let mut exporter = JsonlExporter::new(&output_file, false);
    exporter.initialize().unwrap();

    let stats = exporter.stats_snapshot().unwrap();
    assert_eq!(stats.exported, 0);
    assert_eq!(stats.failed, 0);
}

#[test]
fn test_jsonl_exporter_overwrite_mode() {
    let test_dir = setup_test_dir("overwrite");
    let output_file = test_dir.join("output.jsonl");

    // 第一次写入
    {
        let mut exporter = JsonlExporter::new(&output_file, true);
        exporter.initialize().unwrap();
        exporter.finalize().unwrap();
    }

    // 第二次写入（覆盖模式）
    {
        let mut exporter = JsonlExporter::new(&output_file, true);
        exporter.initialize().unwrap();
        exporter.finalize().unwrap();
    }

    assert!(output_file.exists());
}

#[test]
fn test_jsonl_exporter_debug_format() {
    let test_dir = setup_test_dir("debug");
    let output_file = test_dir.join("output.jsonl");

    let exporter = JsonlExporter::new(&output_file, false);
    let debug_str = format!("{exporter:?}");

    assert!(debug_str.contains("JsonlExporter"));
    assert!(debug_str.contains("path"));
}

#[test]
fn test_jsonl_exporter_nested_directory() {
    let test_dir = setup_test_dir("nested");
    let nested_path = test_dir.join("a").join("b").join("c");
    let output_file = nested_path.join("output.jsonl");

    let mut exporter = JsonlExporter::new(&output_file, false);
    let result = exporter.initialize();

    assert!(result.is_ok());
    assert!(output_file.exists());
}

#[test]
fn test_jsonl_exporter_multiple_initialize() {
    let test_dir = setup_test_dir("multi_init");
    let output_file = test_dir.join("output.jsonl");

    let mut exporter = JsonlExporter::new(&output_file, false);
    exporter.initialize().unwrap();

    // 第二次初始化应该也成功
    let result = exporter.initialize();
    assert!(result.is_ok());
}

#[test]
fn test_jsonl_exporter_finalize_without_initialize() {
    let test_dir = setup_test_dir("no_init");
    let output_file = test_dir.join("output.jsonl");

    let mut exporter = JsonlExporter::new(&output_file, false);

    // finalize 不需要初始化，应该成功
    let result = exporter.finalize();
    assert!(result.is_ok());
}

#[test]
fn test_jsonl_exporter_multiple_finalize() {
    let test_dir = setup_test_dir("multi_final");
    let output_file = test_dir.join("output.jsonl");

    let mut exporter = JsonlExporter::new(&output_file, false);
    exporter.initialize().unwrap();

    exporter.finalize().unwrap();

    // 第二次 finalize 应该也成功
    let result = exporter.finalize();
    assert!(result.is_ok());
}

#[test]
fn test_jsonl_exporter_config_priority() {
    let test_dir = setup_test_dir("priority");
    let output_file = test_dir.join("output.jsonl");

    // append 模式优先级高于 overwrite
    let config = config::JsonlExporter {
        file: output_file.to_str().unwrap().to_string(),
        overwrite: true, // 这个会被忽略
        append: true,    // append 优先
    };

    let mut exporter = JsonlExporter::from_config(&config);
    exporter.initialize().unwrap();
    assert!(output_file.exists());
}

#[test]
fn test_jsonl_exporter_from_config_overwrite() {
    let test_dir = setup_test_dir("config_overwrite");
    let output_file = test_dir.join("output.jsonl");

    let config = config::JsonlExporter {
        file: output_file.to_str().unwrap().to_string(),
        overwrite: true,
        append: false,
    };

    let mut exporter = JsonlExporter::from_config(&config);
    exporter.initialize().unwrap();
    exporter.finalize().unwrap();

    assert!(output_file.exists());
}

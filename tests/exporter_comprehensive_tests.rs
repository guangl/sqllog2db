/// Comprehensive exporter module tests
#[cfg(feature = "csv")]
use dm_database_sqllog2db::config::CsvExporter;
#[cfg(feature = "jsonl")]
use dm_database_sqllog2db::config::JsonlExporter;
#[cfg(feature = "parquet")]
use dm_database_sqllog2db::config::ParquetExporter;

// ==================== CsvExporter Creation Tests ====================

#[cfg(feature = "csv")]
#[test]
fn test_csv_exporter_new() {
    let exporter = CsvExporter {
        file: "output.csv".to_string(),
        overwrite: false,
        append: false,
    };

    assert_eq!(exporter.file, "output.csv");
    assert!(!exporter.overwrite);
    assert!(!exporter.append);
}

#[test]
fn test_csv_exporter_with_directory_path() {
    let exporter = CsvExporter {
        file: "export/data/output.csv".to_string(),
        overwrite: false,
        append: false,
    };

    assert!(exporter.file.contains("export"));
    assert!(exporter.file.contains("csv"));
}

#[test]
fn test_csv_exporter_overwrite_mode() {
    let exporter = CsvExporter {
        file: "output.csv".to_string(),
        overwrite: true,
        append: false,
    };

    assert!(exporter.overwrite);
    assert!(!exporter.append);
}

#[test]
fn test_csv_exporter_append_mode() {
    let exporter = CsvExporter {
        file: "output.csv".to_string(),
        overwrite: false,
        append: true,
    };

    assert!(!exporter.overwrite);
    assert!(exporter.append);
}

#[test]
fn test_csv_exporter_with_absolute_path() {
    let exporter = CsvExporter {
        file: "/var/export/output.csv".to_string(),
        overwrite: false,
        append: false,
    };

    assert!(exporter.file.starts_with('/'));
}

#[test]
fn test_csv_exporter_with_windows_path() {
    let exporter = CsvExporter {
        file: "C:\\export\\output.csv".to_string(),
        overwrite: false,
        append: false,
    };

    assert!(exporter.file.contains("export"));
}

#[test]
fn test_csv_exporter_with_special_characters_in_filename() {
    let exporter = CsvExporter {
        file: "export/2024-12-06_batch_001.csv".to_string(),
        overwrite: false,
        append: false,
    };

    assert!(exporter.file.contains("2024"));
    assert!(exporter.file.contains("batch"));
}

// ==================== JsonlExporter Tests ====================

#[cfg(feature = "jsonl")]
#[test]
fn test_jsonl_exporter_new() {
    let exporter = JsonlExporter {
        file: "output.jsonl".to_string(),
        overwrite: false,
        append: false,
    };

    assert_eq!(exporter.file, "output.jsonl");
    assert!(!exporter.overwrite);
    assert!(!exporter.append);
}

#[cfg(feature = "jsonl")]
#[test]
fn test_jsonl_exporter_with_nested_path() {
    let exporter = JsonlExporter {
        file: "data/export/logs.jsonl".to_string(),
        overwrite: false,
        append: false,
    };

    assert!(exporter.file.contains("jsonl"));
}

#[cfg(feature = "jsonl")]
#[test]
fn test_jsonl_exporter_append_mode() {
    let exporter = JsonlExporter {
        file: "output.jsonl".to_string(),
        overwrite: false,
        append: true,
    };

    assert!(!exporter.overwrite);
    assert!(exporter.append);
}

// ==================== ParquetExporter Tests ====================

#[cfg(feature = "parquet")]
#[test]
fn test_parquet_exporter_new() {
    let exporter = ParquetExporter {
        file: "output.parquet".to_string(),
        overwrite: false,
        row_group_size: Some(1024),
        use_dictionary: Some(true),
    };

    assert_eq!(exporter.file, "output.parquet");
    assert_eq!(exporter.row_group_size, Some(1024));
    assert_eq!(exporter.use_dictionary, Some(true));
}

// ==================== Multiple Exporter Configurations ====================

#[cfg(all(feature = "csv", feature = "jsonl"))]
#[test]
fn test_multiple_exporters_different_formats() {
    let csv = CsvExporter {
        file: "output.csv".to_string(),
        overwrite: false,
        append: false,
    };

    let jsonl = JsonlExporter {
        file: "output.jsonl".to_string(),
        overwrite: false,
        append: false,
    };

    assert!(csv.file.contains("csv"));
    assert!(jsonl.file.contains("jsonl"));
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_file_path_accessors() {
    let csv = CsvExporter {
        file: "/export/data.csv".to_string(),
        overwrite: false,
        append: false,
    };

    let path_str = &csv.file;
    assert!(path_str.contains("export"));
    assert!(path_str.contains("csv"));
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_overwrite_and_append_flags() {
    // Test that flags can be independently set
    let mut configs = vec![CsvExporter {
        file: "1.csv".to_string(),
        overwrite: true,
        append: false,
    }];

    // Add more exporters

    // Only append
    configs.push(CsvExporter {
        file: "2.csv".to_string(),
        overwrite: false,
        append: true,
    });

    // Neither
    configs.push(CsvExporter {
        file: "3.csv".to_string(),
        overwrite: false,
        append: false,
    });

    // Both (edge case)
    configs.push(CsvExporter {
        file: "4.csv".to_string(),
        overwrite: true,
        append: true,
    });

    assert!(configs[0].overwrite);
    assert!(!configs[0].append);
    assert!(!configs[1].overwrite);
    assert!(configs[1].append);
    assert!(!configs[2].overwrite && !configs[2].append);
    assert!(configs[3].overwrite && configs[3].append);
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_file_naming_patterns() {
    let patterns = vec![
        "output.csv",
        "data_2024.csv",
        "batch_001_final.csv",
        "export_20241206_001.csv",
    ];

    for pattern in patterns {
        let exporter = CsvExporter {
            file: pattern.to_string(),
            overwrite: false,
            append: false,
        };

        assert_eq!(exporter.file, pattern);
    }
}

#[test]
fn test_exporter_extension_matching() {
    let csv_files = vec!["output.csv", "data.CSV", "log_export.csv"];
    let jsonl_files = vec!["output.jsonl", "data.jsonl"];

    for csv in csv_files {
        assert!(csv.ends_with("csv") || csv.ends_with("CSV"));
    }

    for jsonl in jsonl_files {
        assert!(jsonl.ends_with("jsonl"));
    }
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_path_with_unicode_characters() {
    let exporter = CsvExporter {
        file: "export/数据_日志.csv".to_string(),
        overwrite: false,
        append: false,
    };

    assert!(exporter.file.contains("日志"));
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_very_long_path() {
    let long_path = "export/".to_string() + &"subdir/".repeat(10) + "output.csv";

    let exporter = CsvExporter {
        file: long_path.clone(),
        overwrite: false,
        append: false,
    };

    assert_eq!(exporter.file, long_path);
    assert!(exporter.file.len() > 50);
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_empty_filename() {
    let exporter = CsvExporter {
        file: String::new(),
        overwrite: false,
        append: false,
    };

    assert_eq!(exporter.file, "");
}

#[cfg(feature = "csv")]
#[test]
fn test_exporter_just_extension() {
    let exporter = CsvExporter {
        file: ".csv".to_string(),
        overwrite: false,
        append: false,
    };

    assert!(exporter.file.contains("csv"));
}

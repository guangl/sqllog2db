/// 导出器性能对比示例程序
use dm_database_parser_sqllog::Sqllog;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use dm_database_sqllog2db::exporter::Exporter;

#[cfg(feature = "csv")]
use dm_database_sqllog2db::exporter::CsvExporter;
#[cfg(feature = "sqlite")]
use dm_database_sqllog2db::exporter::database::SQLiteExporter;

fn main() {
    println!("\n{:=^80}", " 导出器性能对比测试 ");

    let test_dir = PathBuf::from("sqllogs");
    if !test_dir.exists() || fs::read_dir(&test_dir).unwrap().count() == 0 {
        eprintln!("\n错误: 未找到测试数据");
        std::process::exit(1);
    }

    // 收集所有日志文件
    let mut log_files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&test_dir).unwrap() {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("log") {
                log_files.push(path);
            }
        }
    }

    println!("找到 {} 个日志文件\n", log_files.len());

    // 先解析所有数据到内存（只计时一次，后续测试重用）
    println!("预加载: 解析所有日志到内存...");
    let parse_start = Instant::now();
    let mut all_records: Vec<Sqllog> = Vec::new();
    for file in &log_files {
        let path = file.to_string_lossy().to_string();
        if let Ok((mut logs, _)) = dm_database_parser_sqllog::parse_records_from_file(&path) {
            all_records.append(&mut logs);
        }
    }
    let parse_elapsed = parse_start.elapsed();
    println!(
        "  解析完成: {} 条记录, {:.2}s, {:.0} 条/秒\n",
        all_records.len(),
        parse_elapsed.as_secs_f64(),
        all_records.len() as f64 / parse_elapsed.as_secs_f64()
    );

    // 测试各个导出器
    test_csv_exporter(&all_records);
    test_sqlite_exporter(&all_records);

    println!("\n{:=^80}", " 完成 ");
}

fn test_csv_exporter(records: &[Sqllog]) {
    #[cfg(feature = "csv")]
    {
        println!("测试 CSV 导出器:");
        let output_path = "bench_output/test.csv";

        // 确保输出目录存在
        if let Some(parent) = std::path::Path::new(output_path).parent() {
            let _ = fs::create_dir_all(parent);
        }

        // 删除旧文件
        let _ = fs::remove_file(output_path);

        let start = Instant::now();
        let mut exporter = CsvExporter::new(output_path, true);

        if let Err(e) = exporter.initialize() {
            eprintln!("  初始化失败: {}", e);
            return;
        }

        let refs: Vec<&Sqllog> = records.iter().collect();
        if let Err(e) = exporter.export_batch(&refs) {
            eprintln!("  导出失败: {}", e);
            return;
        }

        if let Err(e) = exporter.finalize() {
            eprintln!("  完成失败: {}", e);
            return;
        }

        let elapsed = start.elapsed();
        println!(
            "  {} 条记录, {:.2}s, {:.0} 条/秒\n",
            records.len(),
            elapsed.as_secs_f64(),
            records.len() as f64 / elapsed.as_secs_f64()
        );
    }

    #[cfg(not(feature = "csv"))]
    {
        let _ = records;
        println!("测试 CSV 导出器: 跳过 (未启用 csv feature)\n");
    }
}
fn test_sqlite_exporter(records: &[Sqllog]) {
    #[cfg(feature = "sqlite")]
    {
        println!("测试 SQLite 导出器:");
        let output_path = "bench_output/test.db";

        if let Some(parent) = std::path::Path::new(output_path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::remove_file(output_path);

        let start = Instant::now();
        let mut exporter = SQLiteExporter::with_batch_size(
            output_path.to_string(),
            "sqllogs".to_string(),
            true,
            10000,
        );

        if let Err(e) = exporter.initialize() {
            eprintln!("  初始化失败: {}", e);
            return;
        }

        let refs: Vec<&Sqllog> = records.iter().collect();
        if let Err(e) = exporter.export_batch(&refs) {
            eprintln!("  导出失败: {}", e);
            return;
        }

        if let Err(e) = exporter.finalize() {
            eprintln!("  完成失败: {}", e);
            return;
        }

        let elapsed = start.elapsed();
        println!(
            "  {} 条记录, {:.2}s, {:.0} 条/秒\n",
            records.len(),
            elapsed.as_secs_f64(),
            records.len() as f64 / elapsed.as_secs_f64()
        );
    }

    #[cfg(not(feature = "sqlite"))]
    {
        let _ = records;
        println!("测试 SQLite 导出器: 跳过 (未启用 sqlite feature)\n");
    }
}

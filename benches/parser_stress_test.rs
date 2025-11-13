/// 压测 dm-database-parser-sqllog 库的三个主要接口
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    println!("\n{:=^80}", " dm-database-parser-sqllog API 压力测试 ");

    let test_dir = PathBuf::from("sqllogs");
    if !test_dir.exists() || fs::read_dir(&test_dir).unwrap().count() == 0 {
        eprintln!("\n错误: 未找到测试数据");
        std::process::exit(1);
    }

    let mut log_files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&test_dir).unwrap() {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("log") {
                log_files.push(path);
            }
        }
    }

    test_for_each(&log_files);
    test_iter(&log_files);
    test_parse(&log_files);
    compare_all(&log_files);

    println!("\n{:=^80}", " 完成 ");
}

fn test_for_each(files: &[PathBuf]) {
    println!("\n测试 1: for_each_sqllog_from_file");
    let mut total = 0;
    let start = Instant::now();

    for file in files {
        let path = file.to_string_lossy().to_string();
        let mut count = 0;
        let _ = dm_database_parser_sqllog::for_each_sqllog_from_file(&path, |_| {
            count += 1;
        });
        total += count;
    }

    println!(
        "  总计: {} 条, {:.2}s, {:.0} 条/秒",
        total,
        start.elapsed().as_secs_f64(),
        total as f64 / start.elapsed().as_secs_f64()
    );
}

fn test_iter(files: &[PathBuf]) {
    println!("\n测试 2: iter_sqllogs_from_file");
    let mut total = 0;
    let start = Instant::now();

    for file in files {
        let path = file.to_string_lossy().to_string();
        if let Ok(iter) = dm_database_parser_sqllog::iter_sqllogs_from_file(&path) {
            total += iter.filter(|r| r.is_ok()).count();
        }
    }

    println!(
        "  总计: {} 条, {:.2}s, {:.0} 条/秒",
        total,
        start.elapsed().as_secs_f64(),
        total as f64 / start.elapsed().as_secs_f64()
    );
}

fn test_parse(files: &[PathBuf]) {
    println!("\n测试 3: parse_sqllogs_from_file");
    let mut total = 0;
    let start = Instant::now();

    for file in files {
        let path = file.to_string_lossy().to_string();
        if let Ok((logs, _)) = dm_database_parser_sqllog::parse_sqllogs_from_file(&path) {
            total += logs.len();
        }
    }

    println!(
        "  总计: {} 条, {:.2}s, {:.0} 条/秒",
        total,
        start.elapsed().as_secs_f64(),
        total as f64 / start.elapsed().as_secs_f64()
    );
}

fn compare_all(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }

    println!("\n对比测试 (3次运行，全部文件):");

    // 1) for_each 全量
    let mut for_each_times: Vec<Duration> = Vec::with_capacity(3);
    let mut for_each_counts: Vec<usize> = Vec::with_capacity(3);
    for _ in 0..3 {
        let start = Instant::now();
        let mut total = 0usize;
        for file in files {
            let path = file.to_string_lossy().to_string();
            let mut c = 0usize;
            let _ = dm_database_parser_sqllog::for_each_sqllog_from_file(&path, |_| {
                c += 1;
            });
            total += c;
        }
        for_each_times.push(start.elapsed());
        for_each_counts.push(total);
    }
    let for_each_avg =
        for_each_times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / for_each_times.len() as f64;
    let for_each_count = *for_each_counts.first().unwrap_or(&0);
    println!(
        "  for_each: {:.3}s ({} 条, {:.0} 条/秒)",
        for_each_avg,
        for_each_count,
        if for_each_avg > 0.0 {
            for_each_count as f64 / for_each_avg
        } else {
            0.0
        }
    );

    // 2) iter 全量
    let mut iter_times: Vec<Duration> = Vec::with_capacity(3);
    let mut iter_counts: Vec<usize> = Vec::with_capacity(3);
    for _ in 0..3 {
        let start = Instant::now();
        let mut total = 0usize;
        for file in files {
            let path = file.to_string_lossy().to_string();
            if let Ok(iter) = dm_database_parser_sqllog::iter_sqllogs_from_file(&path) {
                total += iter.filter(|r| r.is_ok()).count();
            }
        }
        iter_times.push(start.elapsed());
        iter_counts.push(total);
    }
    let iter_avg =
        iter_times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / iter_times.len() as f64;
    let iter_count = *iter_counts.first().unwrap_or(&0);
    println!(
        "  iter:     {:.3}s ({} 条, {:.0} 条/秒)",
        iter_avg,
        iter_count,
        if iter_avg > 0.0 {
            iter_count as f64 / iter_avg
        } else {
            0.0
        }
    );

    // 3) parse 全量
    let mut parse_times: Vec<Duration> = Vec::with_capacity(3);
    let mut parse_counts: Vec<usize> = Vec::with_capacity(3);
    for _ in 0..3 {
        let start = Instant::now();
        let mut total = 0usize;
        for file in files {
            let path = file.to_string_lossy().to_string();
            if let Ok((logs, _)) = dm_database_parser_sqllog::parse_sqllogs_from_file(&path) {
                total += logs.len();
            }
        }
        parse_times.push(start.elapsed());
        parse_counts.push(total);
    }
    let parse_avg =
        parse_times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / parse_times.len() as f64;
    let parse_count = *parse_counts.first().unwrap_or(&0);
    println!(
        "  parse:    {:.3}s ({} 条, {:.0} 条/秒)",
        parse_avg,
        parse_count,
        if parse_avg > 0.0 {
            parse_count as f64 / parse_avg
        } else {
            0.0
        }
    );
}

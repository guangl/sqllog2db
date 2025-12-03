/// 解析性能基准测试：处理 sqllogs 文件夹中的所有日志文件
/// 
/// 功能:
/// 1. 扫描 sqllogs 目录下所有 .log 文件
/// 2. 解析所有日志记录
/// 3. 统计解析性能指标
/// 
/// 运行方式:
/// cargo run --release --example parse_benchmark

use dm_database_parser_sqllog::LogParser;
use std::fs;
use std::path::Path;
use std::time::Instant;

fn main() {
    println!("=== SQL 日志解析性能基准测试 ===\n");

    let log_dir = "sqllogs";
    
    if !Path::new(log_dir).exists() {
        eprintln!("❌ 目录不存在: {}", log_dir);
        return;
    }

    // 扫描所有 .log 文件
    let log_files = match scan_log_files(log_dir) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("❌ 扫描目录失败: {}", e);
            return;
        }
    };

    if log_files.is_empty() {
        eprintln!("❌ 未找到任何 .log 文件");
        return;
    }

    println!("📁 找到 {} 个日志文件:\n", log_files.len());
    for (i, file) in log_files.iter().enumerate() {
        println!("   {}. {}", i + 1, file);
    }
    println!();

    // 总体统计
    let mut total_records = 0u64;
    let mut total_errors = 0u64;
    let mut total_bytes = 0u64;

    let overall_start = Instant::now();

    // 处理每个文件
    for (file_idx, log_file) in log_files.iter().enumerate() {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📄 文件 {}/{}: {}", file_idx + 1, log_files.len(), log_file);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // 获取文件大小
        if let Ok(metadata) = fs::metadata(log_file) {
            let file_size = metadata.len();
            total_bytes += file_size;
            println!("📊 文件大小: {:.2} MB ({} bytes)", 
                     file_size as f64 / 1024.0 / 1024.0, file_size);
        }

        // 创建解析器
        let parser = match LogParser::from_path(log_file) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("❌ 创建解析器失败: {}", e);
                continue;
            }
        };

        let mut file_records = 0u64;
        let mut file_errors = 0u64;
        let file_start = Instant::now();

        // 解析所有记录
        for result in parser.iter() {
            match result {
                Ok(_sqllog) => {
                    file_records += 1;

                    // 每处理 10 万条记录显示进度
                    if file_records % 100000 == 0 {
                        let elapsed = file_start.elapsed().as_secs_f64();
                        let speed = file_records as f64 / elapsed;
                        println!("   进度: {} 条记录 ({:.0} 条/秒)", file_records, speed);
                    }
                }
                Err(_e) => {
                    file_errors += 1;
                }
            }
        }

        let file_elapsed = file_start.elapsed();

        // 文件统计
        total_records += file_records;
        total_errors += file_errors;

        println!("\n✅ 文件处理完成:");
        println!("   成功解析:     {:>12} 条", file_records);
        println!("   解析错误:     {:>12} 条", file_errors);
        println!("   总计:         {:>12} 条", file_records + file_errors);
        println!("   耗时:         {:>12.2} 秒", file_elapsed.as_secs_f64());
        println!("   速度:         {:>12.0} 条/秒", 
                 file_records as f64 / file_elapsed.as_secs_f64());
        println!();
    }

    let overall_elapsed = overall_start.elapsed();

    // 总体统计报告
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📈 总体统计报告");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\n📁 文件统计:");
    println!("   处理文件数:   {:>12} 个", log_files.len());
    println!("   总文件大小:   {:>12.2} MB", total_bytes as f64 / 1024.0 / 1024.0);
    println!("   总文件大小:   {:>12.2} GB", total_bytes as f64 / 1024.0 / 1024.0 / 1024.0);

    println!("\n🔢 记录统计:");
    println!("   成功解析:     {:>12} 条", total_records);
    println!("   解析错误:     {:>12} 条", total_errors);
    println!("   总计:         {:>12} 条", total_records + total_errors);
    println!("   成功率:       {:>11.2}%", 
             if total_records + total_errors > 0 {
                 (total_records as f64 / (total_records + total_errors) as f64) * 100.0
             } else {
                 0.0
             });

    println!("\n⏱️  性能统计:");
    println!("   总耗时:       {:>12.2} 秒", overall_elapsed.as_secs_f64());
    println!("   解析速度:     {:>12.0} 条/秒", 
             total_records as f64 / overall_elapsed.as_secs_f64());
    println!("   数据吞吐量:   {:>12.2} MB/秒", 
             (total_bytes as f64 / 1024.0 / 1024.0) / overall_elapsed.as_secs_f64());

    // 如果总耗时超过 1 秒，显示更多时间格式
    if overall_elapsed.as_secs() > 0 {
        let hours = overall_elapsed.as_secs() / 3600;
        let minutes = (overall_elapsed.as_secs() % 3600) / 60;
        let seconds = overall_elapsed.as_secs() % 60;
        let millis = overall_elapsed.subsec_millis();

        print!("   耗时 (H:M:S): {:>12}", 
               if hours > 0 {
                   format!("{}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
               } else if minutes > 0 {
                   format!("{}:{:02}.{:03}", minutes, seconds, millis)
               } else {
                   format!("{}.{:03} 秒", seconds, millis)
               });
        println!();
    }

    println!("\n✅ 基准测试完成!");
}

/// 扫描目录下所有 .log 文件
fn scan_log_files(dir: &str) -> Result<Vec<String>, std::io::Error> {
    let mut log_files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "log" {
                    if let Some(path_str) = path.to_str() {
                        log_files.push(path_str.to_string());
                    }
                }
            }
        }
    }

    // 按文件名排序
    log_files.sort();

    Ok(log_files)
}

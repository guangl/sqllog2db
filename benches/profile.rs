use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    println!("\n{:=^60}", " Performance Profiling ");
    println!("\nThis tool measures time spent in different stages.\n");

    // Check test data
    let sqllog_path = PathBuf::from("sqllogs");
    if !sqllog_path.exists() {
        eprintln!("Error: No test data found in sqllogs/");
        std::process::exit(1);
    }

    println!("Test data:");
    for entry in fs::read_dir(&sqllog_path).unwrap() {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("log") {
                let size = entry.metadata().unwrap().len();
                println!(
                    "  - {} ({:.2} MB)",
                    path.file_name().unwrap().to_str().unwrap(),
                    size as f64 / 1_048_576.0
                );
            }
        }
    }

    println!("\n{:-<60}", "");
    println!("Stage 1: Parse-only benchmark (no export)");
    println!("{:-<60}", "");

    let start = Instant::now();
    let mut total_records = 0;

    for entry in fs::read_dir(&sqllog_path).unwrap() {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("log") {
                let path_str = path.to_string_lossy().to_string();

                if let Ok(iter) = dm_database_parser_sqllog::iter_sqllogs_from_file(&path_str) {
                    for result in iter {
                        if result.is_ok() {
                            total_records += 1;
                        }
                    }
                }
            }
        }
    }

    let parse_only_time = start.elapsed();
    println!("  Total records: {}", total_records);
    println!("  Parse time: {:.2}s", parse_only_time.as_secs_f64());
    println!(
        "  Throughput: {:.0} records/s",
        total_records as f64 / parse_only_time.as_secs_f64()
    );

    println!("\n{:-<60}", "");
    println!("Stage 2: Parse + in-memory collect (no file I/O)");
    println!("{:-<60}", "");

    let start = Instant::now();
    let mut records = Vec::new();

    for entry in fs::read_dir(&sqllog_path).unwrap() {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("log") {
                let path_str = path.to_string_lossy().to_string();

                if let Ok(iter) = dm_database_parser_sqllog::iter_sqllogs_from_file(&path_str) {
                    for result in iter {
                        if let Ok(record) = result {
                            records.push(record);
                        }
                    }
                }
            }
        }
    }

    let collect_time = start.elapsed();
    println!("  Collected records: {}", records.len());
    println!("  Parse + collect time: {:.2}s", collect_time.as_secs_f64());
    let overhead = if collect_time > parse_only_time {
        format!("+{:.2}s", (collect_time - parse_only_time).as_secs_f64())
    } else {
        format!(
            "-{:.2}s (cached)",
            (parse_only_time - collect_time).as_secs_f64()
        )
    };
    println!("  Overhead vs parse-only: {}", overhead);

    println!("\n{:-<60}", "");
    println!("Stage 3: CSV formatting (no file write)");
    println!("{:-<60}", "");

    let start = Instant::now();
    let mut total_bytes = 0;

    // CSV header (matching actual CSV format)
    let header = "ts,ep,sess_id,thrd_id,username,trxid,statement,appname,client_ip,sql,exec_time,row_count,exec_id\n";
    total_bytes += header.len();

    for record in &records {
        // Simulate CSV line formatting
        let exec_time = record
            .indicators
            .as_ref()
            .map(|i| i.execute_time.to_string())
            .unwrap_or_default();
        let row_count = record
            .indicators
            .as_ref()
            .map(|i| i.row_count.to_string())
            .unwrap_or_default();
        let exec_id = record
            .indicators
            .as_ref()
            .map(|i| i.execute_id.to_string())
            .unwrap_or_default();

        let line = format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.ts,
            record.meta.ep,
            record.meta.sess_id,
            record.meta.thrd_id,
            record.meta.username,
            record.meta.trxid,
            record.meta.statement,
            record.meta.appname,
            record.meta.client_ip,
            record.body.replace('\n', " ").replace(',', ";"),
            exec_time,
            row_count,
            exec_id
        );
        total_bytes += line.len();
    }

    let format_time = start.elapsed();
    println!(
        "  Formatted bytes: {:.2} MB",
        total_bytes as f64 / 1_048_576.0
    );
    println!("  Format time: {:.2}s", format_time.as_secs_f64());
    println!(
        "  Format speed: {:.2} MB/s",
        total_bytes as f64 / 1_048_576.0 / format_time.as_secs_f64()
    );

    println!("\n{:-<60}", "");
    println!("Stage 4: Full pipeline with file write");
    println!("{:-<60}", "");

    let output_path = "profile-test.csv";
    let _ = fs::remove_file(output_path);

    let start = Instant::now();

    let mut writer = std::io::BufWriter::with_capacity(
        1024 * 1024, // 1MB buffer
        fs::File::create(output_path).unwrap(),
    );

    use std::io::Write;
    writeln!(writer, "{}", header.trim()).unwrap();

    for record in &records {
        let exec_time = record
            .indicators
            .as_ref()
            .map(|i| i.execute_time.to_string())
            .unwrap_or_default();
        let row_count = record
            .indicators
            .as_ref()
            .map(|i| i.row_count.to_string())
            .unwrap_or_default();
        let exec_id = record
            .indicators
            .as_ref()
            .map(|i| i.execute_id.to_string())
            .unwrap_or_default();

        let line = format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.ts,
            record.meta.ep,
            record.meta.sess_id,
            record.meta.thrd_id,
            record.meta.username,
            record.meta.trxid,
            record.meta.statement,
            record.meta.appname,
            record.meta.client_ip,
            record.body.replace('\n', " ").replace(',', ";"),
            exec_time,
            row_count,
            exec_id
        );
        write!(writer, "{}", line).unwrap();
    }

    writer.flush().unwrap();
    drop(writer);

    let full_time = start.elapsed();
    let file_size = fs::metadata(output_path).unwrap().len();

    println!("  Output file: {:.2} MB", file_size as f64 / 1_048_576.0);
    println!("  Total time: {:.2}s", full_time.as_secs_f64());
    println!(
        "  Write speed: {:.2} MB/s",
        file_size as f64 / 1_048_576.0 / full_time.as_secs_f64()
    );

    // Clean up
    let _ = fs::remove_file(output_path);

    println!("\n{:=^60}", " Summary ");
    println!("\n{:<30} {:>10} {:>15}", "Stage", "Time (s)", "Overhead");
    println!("{:-<60}", "");
    println!(
        "{:<30} {:>10.2} {:>15}",
        "1. Parse only",
        parse_only_time.as_secs_f64(),
        "baseline"
    );
    println!(
        "{:<30} {:>10.2} {:>15.2}",
        "2. Parse + collect",
        collect_time.as_secs_f64(),
        format!("+{:.2}s", (collect_time - parse_only_time).as_secs_f64())
    );
    println!(
        "{:<30} {:>10.2} {:>15.2}",
        "3. CSV format",
        format_time.as_secs_f64(),
        ""
    );
    let write_overhead = if full_time > format_time {
        format!("+{:.2}s", (full_time - format_time).as_secs_f64())
    } else {
        format!("{:.2}s", full_time.as_secs_f64())
    };
    println!(
        "{:<30} {:>10.2} {:>15}",
        "4. Full pipeline",
        full_time.as_secs_f64(),
        write_overhead
    );

    println!("\n{:=^60}\n", " Profiling Complete ");
}

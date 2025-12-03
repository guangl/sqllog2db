/// CSV 导出性能分析工具
///
/// 分阶段测试 CSV 导出各个环节的性能消耗:
/// 1. 纯解析性能
/// 2. 解析 + 字符串格式化
/// 3. 解析 + 格式化 + 文件写入(to_string)
/// 4. 解析 + 格式化 + 文件写入(itoa优化)
///
/// 运行方式:
/// cargo run --release --example csv_performance_analysis
use dm_database_parser_sqllog::LogParser;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

fn main() {
    let log_file = r"D:\code\sqllog2db\sqllogs\dmsql_OASIS_DB1_20251020_151030.log";

    println!("=== CSV 性能分析 ===\n");

    // ========== 阶段 1: 纯解析性能 ==========
    println!("【阶段 1】纯解析性能测试");
    let start = Instant::now();
    let parser = LogParser::from_path(log_file).expect("Failed to create parser");
    let mut parse_count = 0;

    for log_result in parser.iter() {
        if let Ok(log) = log_result {
            let _meta = log.parse_meta();
            let _indicators = log.parse_indicators();
            let _body = log.body();
            parse_count += 1;
        }
    }

    let parse_duration = start.elapsed();
    println!("  - 解析记录数: {}", parse_count);
    println!("  - 解析耗时: {:.3} 秒", parse_duration.as_secs_f64());
    println!(
        "  - 解析速度: {:.2} M records/sec\n",
        parse_count as f64 / parse_duration.as_secs_f64() / 1_000_000.0
    );

    // ========== 阶段 2: 解析 + 字符串拼接（不写文件） ==========
    println!("【阶段 2】解析 + CSV 字符串拼接（内存）");
    let start = Instant::now();
    let parser = LogParser::from_path(log_file).expect("Failed to create parser");
    let mut buffer = String::with_capacity(1024 * 1024); // 1MB buffer
    let mut line_buf = String::with_capacity(512);
    let mut format_count = 0;

    for log_result in parser.iter() {
        if let Ok(log) = log_result {
            let meta = log.parse_meta();
            line_buf.clear();

            // 时间戳
            line_buf.push_str(log.ts.as_ref());
            line_buf.push(',');

            // ep
            line_buf.push_str(&meta.ep.to_string());
            line_buf.push(',');

            // 其他字段
            line_buf.push_str(meta.sess_id.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.thrd_id.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.username.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.trxid.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.statement.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.appname.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.client_ip.as_ref());
            line_buf.push(',');
            line_buf.push_str(log.body().as_ref());
            line_buf.push(',');

            if let Some(indicators) = log.parse_indicators() {
                line_buf.push_str(&(indicators.execute_time as i64).to_string());
                line_buf.push(',');
                line_buf.push_str(&(indicators.row_count as i64).to_string());
                line_buf.push(',');
                line_buf.push_str(&indicators.execute_id.to_string());
                line_buf.push('\n');
            } else {
                line_buf.push_str(",,\n");
            }

            buffer.push_str(&line_buf);
            format_count += 1;

            // 每 10000 条清空一次缓冲区（模拟批量写入）
            if format_count % 10000 == 0 {
                buffer.clear();
            }
        }
    }

    let format_duration = start.elapsed();
    println!("  - 格式化记录数: {}", format_count);
    println!("  - 总耗时: {:.3} 秒", format_duration.as_secs_f64());
    println!(
        "  - 格式化开销: {:.3} 秒",
        (format_duration - parse_duration).as_secs_f64()
    );
    println!(
        "  - 格式化速度: {:.2} M records/sec\n",
        format_count as f64 / format_duration.as_secs_f64() / 1_000_000.0
    );

    // ========== 阶段 3: 解析 + 格式化 + 写文件（to_string） ==========
    println!("【阶段 3】解析 + 格式化 + 批量写文件(to_string)");
    let start = Instant::now();
    let parser = LogParser::from_path(log_file).expect("Failed to create parser");
    let output_file = "export/perf_test.csv";
    std::fs::create_dir_all("export").ok();

    let file = File::create(output_file).expect("Failed to create file");
    let mut writer = BufWriter::with_capacity(16 * 1024 * 1024, file); // 16MB buffer

    // 写入表头
    writer.write_all(b"ts,ep,sess_id,thrd_id,username,trx_id,statement,appname,client_ip,sql,exec_time_ms,row_count,exec_id\n").unwrap();

    let mut batch_buf = String::with_capacity(1024 * 1024); // 1MB batch buffer
    let mut line_buf = String::with_capacity(512);
    let mut write_count = 0;
    let batch_threshold = 10000;

    for log_result in parser.iter() {
        if let Ok(log) = log_result {
            let meta = log.parse_meta();
            line_buf.clear();

            // 格式化 CSV 行
            line_buf.push_str(log.ts.as_ref());
            line_buf.push(',');
            line_buf.push_str(&meta.ep.to_string());
            line_buf.push(',');
            line_buf.push_str(meta.sess_id.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.thrd_id.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.username.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.trxid.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.statement.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.appname.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.client_ip.as_ref());
            line_buf.push(',');
            line_buf.push_str(log.body().as_ref());
            line_buf.push(',');

            if let Some(indicators) = log.parse_indicators() {
                line_buf.push_str(&(indicators.execute_time as i64).to_string());
                line_buf.push(',');
                line_buf.push_str(&(indicators.row_count as i64).to_string());
                line_buf.push(',');
                line_buf.push_str(&indicators.execute_id.to_string());
                line_buf.push('\n');
            } else {
                line_buf.push_str(",,\n");
            }

            batch_buf.push_str(&line_buf);
            write_count += 1;

            // 批量写入
            if write_count % batch_threshold == 0 {
                writer.write_all(batch_buf.as_bytes()).unwrap();
                batch_buf.clear();
            }
        }
    }

    // 写入剩余数据
    if !batch_buf.is_empty() {
        writer.write_all(batch_buf.as_bytes()).unwrap();
    }
    writer.flush().unwrap();

    let write_duration = start.elapsed();
    println!("  - 写入记录数: {}", write_count);
    println!("  - 总耗时: {:.3} 秒", write_duration.as_secs_f64());
    println!(
        "  - 文件写入开销: {:.3} 秒",
        (write_duration - format_duration).as_secs_f64()
    );
    println!(
        "  - 整体速度: {:.2} M records/sec\n",
        write_count as f64 / write_duration.as_secs_f64() / 1_000_000.0
    );

    // ========== 阶段 4: 使用 itoa 优化整数转换 ==========
    println!("【阶段 4】使用 itoa 优化整数转换");
    let start = Instant::now();
    let parser = LogParser::from_path(log_file).expect("Failed to create parser");

    let file = File::create("export/perf_test_itoa.csv").expect("Failed to create file");
    let mut writer = BufWriter::with_capacity(16 * 1024 * 1024, file);
    writer.write_all(b"ts,ep,sess_id,thrd_id,username,trx_id,statement,appname,client_ip,sql,exec_time_ms,row_count,exec_id\n").unwrap();

    let mut batch_buf = String::with_capacity(1024 * 1024);
    let mut line_buf = String::with_capacity(512);
    let mut itoa_buf = itoa::Buffer::new();
    let mut itoa_count = 0;

    for log_result in parser.iter() {
        if let Ok(log) = log_result {
            let meta = log.parse_meta();
            line_buf.clear();

            line_buf.push_str(log.ts.as_ref());
            line_buf.push(',');
            line_buf.push_str(itoa_buf.format(meta.ep)); // 使用 itoa
            line_buf.push(',');
            line_buf.push_str(meta.sess_id.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.thrd_id.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.username.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.trxid.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.statement.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.appname.as_ref());
            line_buf.push(',');
            line_buf.push_str(meta.client_ip.as_ref());
            line_buf.push(',');
            line_buf.push_str(log.body().as_ref());
            line_buf.push(',');

            if let Some(indicators) = log.parse_indicators() {
                line_buf.push_str(itoa_buf.format(indicators.execute_time as i64)); // 使用 itoa
                line_buf.push(',');
                line_buf.push_str(itoa_buf.format(indicators.row_count as i64)); // 使用 itoa
                line_buf.push(',');
                line_buf.push_str(itoa_buf.format(indicators.execute_id)); // 使用 itoa
                line_buf.push('\n');
            } else {
                line_buf.push_str(",,\n");
            }

            batch_buf.push_str(&line_buf);
            itoa_count += 1;

            if itoa_count % batch_threshold == 0 {
                writer.write_all(batch_buf.as_bytes()).unwrap();
                batch_buf.clear();
            }
        }
    }

    if !batch_buf.is_empty() {
        writer.write_all(batch_buf.as_bytes()).unwrap();
    }
    writer.flush().unwrap();

    let itoa_duration = start.elapsed();
    println!("  - 写入记录数: {}", itoa_count);
    println!("  - 总耗时: {:.3} 秒", itoa_duration.as_secs_f64());
    println!(
        "  - itoa 优化节省: {:.3} 秒",
        (write_duration - itoa_duration).as_secs_f64()
    );
    println!(
        "  - 整体速度: {:.2} M records/sec\n",
        itoa_count as f64 / itoa_duration.as_secs_f64() / 1_000_000.0
    );

    // ========== 总结 ==========
    println!("=== 性能分析总结 ===");
    println!(
        "纯解析耗时:          {:.3} 秒 (基准 100%)",
        parse_duration.as_secs_f64()
    );
    println!(
        "+ 字符串格式化:      {:.3} 秒 (增加 {:.1}%)",
        format_duration.as_secs_f64(),
        (format_duration.as_secs_f64() / parse_duration.as_secs_f64() - 1.0) * 100.0
    );
    println!(
        "+ 文件写入(to_string): {:.3} 秒 (增加 {:.1}%)",
        write_duration.as_secs_f64(),
        (write_duration.as_secs_f64() / parse_duration.as_secs_f64() - 1.0) * 100.0
    );
    println!(
        "+ 文件写入(itoa):    {:.3} 秒 (增加 {:.1}%)",
        itoa_duration.as_secs_f64(),
        (itoa_duration.as_secs_f64() / parse_duration.as_secs_f64() - 1.0) * 100.0
    );
    println!("\n各阶段开销分析:");
    println!("  - 纯解析:        {:.3} 秒", parse_duration.as_secs_f64());
    println!(
        "  - 格式化开销:    {:.3} 秒 ({:.1}%)",
        (format_duration - parse_duration).as_secs_f64(),
        (format_duration - parse_duration).as_secs_f64() / parse_duration.as_secs_f64() * 100.0
    );
    println!(
        "  - 写入开销(std):  {:.3} 秒 ({:.1}%)",
        (write_duration - format_duration).as_secs_f64(),
        (write_duration - format_duration).as_secs_f64() / parse_duration.as_secs_f64() * 100.0
    );
    println!(
        "  - 写入开销(itoa): {:.3} 秒 ({:.1}%)",
        (itoa_duration - parse_duration).as_secs_f64(),
        (itoa_duration - parse_duration).as_secs_f64() / parse_duration.as_secs_f64() * 100.0
    );
    println!("\nitoa vs to_string:");
    println!(
        "  - 节省时间: {:.3} 秒",
        (write_duration - itoa_duration).as_secs_f64()
    );
    println!(
        "  - 性能提升: {:.1}%",
        (write_duration.as_secs_f64() / itoa_duration.as_secs_f64() - 1.0) * 100.0
    );
}

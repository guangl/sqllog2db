use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn create_test_config(batch_size: usize, name: &str) -> PathBuf {
    let config_content = format!(
        r#"[sqllog]
path = "sqllogs"
batch_size = {}

[error]
path = "errors-bench.jsonl"

[logging]
path = "logs/bench.log"
level = "warn"
retention_days = 1

[features]
replace_sql_parameters = false
scatter = false

[exporter.csv]
path = "export/bench-{}.csv"
overwrite = true
"#,
        batch_size, name
    );

    let config_path = PathBuf::from(format!("bench-config-{}.toml", name));
    fs::write(&config_path, config_content).expect("Failed to write config");
    config_path
}

fn cleanup_test_files(config_path: &PathBuf, output_path: &str) {
    let _ = fs::remove_file(config_path);
    let _ = fs::remove_file(output_path);
    let _ = fs::remove_file("errors-bench.jsonl");
}

fn run_benchmark(batch_size: usize, name: &str, runs: usize) -> (Duration, Duration, Duration) {
    println!("\n{:=<60}", "=");
    println!("Benchmark: {} (batch_size = {})", name, batch_size);
    println!("{:=<60}", "=");

    let config_path = create_test_config(batch_size, name);
    let output_path = format!("export/bench-{}.csv", name);

    let mut times = Vec::new();

    for run in 1..=runs {
        // Clean up before each run
        let _ = fs::remove_file(&output_path);

        print!("  Run {}/{}: ", run, runs);
        std::io::Write::flush(&mut std::io::stdout()).unwrap();

        let start = Instant::now();
        let status = std::process::Command::new("target/release/sqllog2db")
            .args(&["run", "--config", config_path.to_str().unwrap()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("Failed to run sqllog2db");

        let elapsed = start.elapsed();

        if status.success() {
            times.push(elapsed);
            println!("{:.2}s", elapsed.as_secs_f64());
        } else {
            println!("FAILED");
        }
    }

    cleanup_test_files(&config_path, &output_path);

    if times.is_empty() {
        panic!("All benchmark runs failed!");
    }

    let total: Duration = times.iter().sum();
    let avg = total / times.len() as u32;
    let min = *times.iter().min().unwrap();
    let max = *times.iter().max().unwrap();

    println!("\n  Average: {:.2}s", avg.as_secs_f64());
    println!("  Min:     {:.2}s", min.as_secs_f64());
    println!("  Max:     {:.2}s", max.as_secs_f64());

    // Calculate throughput if output file exists
    if let Ok(metadata) = fs::metadata(&output_path) {
        let size_mb = metadata.len() as f64 / 1_048_576.0;
        println!("  Output:  {:.2} MB", size_mb);
    }

    (avg, min, max)
}

fn main() {
    println!("\n{:=^60}", " sqllog2db Performance Benchmark ");
    println!("\nBuilding release binary...");

    // Ensure release build
    let build_status = std::process::Command::new("cargo")
        .args(&["build", "--release"])
        .stdout(std::process::Stdio::null())
        .status()
        .expect("Failed to build release");

    if !build_status.success() {
        eprintln!("Build failed!");
        std::process::exit(1);
    }

    println!("Build complete.\n");

    // Check if test data exists
    let sqllog_path = PathBuf::from("sqllogs");
    if !sqllog_path.exists() || fs::read_dir(&sqllog_path).unwrap().count() == 0 {
        eprintln!("Error: No test data found in sqllogs/ directory");
        std::process::exit(1);
    }

    // Count records in test data
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

    let runs = 3;

    // Run benchmarks with different batch sizes
    let configs = vec![(1000, "1k"), (10000, "10k"), (50000, "50k"), (0, "all")];

    let mut results = Vec::new();

    for (batch_size, name) in configs {
        let (avg, min, _max) = run_benchmark(batch_size, name, runs);
        results.push((name, batch_size, avg, min));
    }

    // Print summary
    println!("\n{:=^60}", " Summary ");
    println!(
        "\n{:<20} {:>12} {:>12} {:>12}",
        "Configuration", "Batch Size", "Avg (s)", "Min (s)"
    );
    println!("{:-<60}", "");

    let mut fastest_time = Duration::MAX;
    let mut fastest_name = "";

    for (name, batch_size, avg, min) in &results {
        let batch_str = if *batch_size == 0 {
            "All".to_string()
        } else {
            format!("{}", batch_size)
        };
        println!(
            "{:<20} {:>12} {:>12.2} {:>12.2}",
            name,
            batch_str,
            avg.as_secs_f64(),
            min.as_secs_f64()
        );

        if *avg < fastest_time {
            fastest_time = *avg;
            fastest_name = name;
        }
    }

    println!(
        "\n🏆 Fastest: {} ({:.2}s)\n",
        fastest_name,
        fastest_time.as_secs_f64()
    );

    // Show relative performance
    println!("Relative Performance (vs fastest):");
    for (name, _batch_size, avg, _min) in &results {
        let relative = (avg.as_secs_f64() / fastest_time.as_secs_f64() * 100.0) as i32;
        let diff = avg.as_secs_f64() - fastest_time.as_secs_f64();
        if name == &fastest_name {
            println!("  {:<20} {:>3}% (baseline)", name, relative);
        } else {
            println!("  {:<20} {:>3}% (+{:.2}s)", name, relative, diff);
        }
    }

    println!("\n{:=^60}\n", " Benchmark Complete ");
}

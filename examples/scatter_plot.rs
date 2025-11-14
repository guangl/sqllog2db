use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::process;

#[cfg(all(feature = "scatter", feature = "sqlite"))]
use dm_database_sqllog2db::features::scatter::{read_stats_from_sqlite, scatter_to_svg};

fn main() -> Result<(), Box<dyn Error>> {
    // 用法 (sqlite): cargo run --example scatter_plot -- <sqlite_path> <table> <svg_out>
    let args: Vec<String> = env::args().collect();

    #[cfg(feature = "scatter")]
    {
        if args.len() != 4 {
            eprintln!("Usage: {} <sqlite_path> <table> <svg_out>", args[0]);
            process::exit(1);
        }
        let sqlite = PathBuf::from(&args[1]);
        let table = &args[2];
        let svg_out = PathBuf::from(&args[3]);
        let stats = read_stats_from_sqlite(&sqlite, table)?;
        scatter_to_svg(&stats, &svg_out)?;
        println!("Wrote scatter SVG to {}", svg_out.display());
    }
    Ok(())
}

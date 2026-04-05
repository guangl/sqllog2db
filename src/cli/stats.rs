use crate::color;
use crate::config::Config;
use crate::parser::SqllogParser;
use dm_database_parser_sqllog::LogParser;
use indicatif::{HumanCount, ProgressBar, ProgressStyle};
use std::time::{Duration, Instant};

pub fn handle_stats(cfg: &Config, quiet: bool) {
    let start = Instant::now();

    let log_files = match SqllogParser::new(&cfg.sqllog.directory).log_files() {
        Ok(files) => files,
        Err(e) => {
            eprintln!("{} {e}", color::red("Error:"));
            return;
        }
    };

    if log_files.is_empty() {
        eprintln!("No log files found in {}", cfg.sqllog.directory);
        return;
    }

    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.cyan} [{prefix}] {msg} | {human_pos} records [{elapsed_precise}]",
            )
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
        );
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    };

    let total_files = log_files.len();
    let mut total_records: u64 = 0;
    let mut total_errors: u64 = 0;

    for (idx, log_file) in log_files.iter().enumerate() {
        let file_name = log_file
            .file_name()
            .map_or_else(|| log_file.to_string_lossy(), |n| n.to_string_lossy());

        pb.set_prefix(format!("{}/{total_files}", idx + 1));
        pb.set_message(file_name.to_string());

        let Ok(parser) = LogParser::from_path(log_file.as_path()) else {
            total_errors += 1;
            continue;
        };

        let mut file_records: u64 = 0;
        let mut file_errors: u64 = 0;
        let filters = cfg.features.filters.as_ref().filter(|f| f.has_filters());
        for result in parser.iter() {
            match result {
                Ok(record) => {
                    if let Some(f) = filters {
                        let meta = record.parse_meta();
                        if !f.should_keep(
                            record.ts.as_ref(),
                            &meta.trxid,
                            &meta.client_ip,
                            &meta.sess_id,
                            &meta.thrd_id,
                            &meta.username,
                            &meta.statement,
                            &meta.appname,
                            record.tag.as_deref(),
                        ) {
                            continue;
                        }
                    }
                    file_records += 1;
                    pb.inc(1);
                }
                Err(_) => file_errors += 1,
            }
        }

        total_records += file_records;
        total_errors += file_errors;
    }

    pb.finish_and_clear();

    if !quiet {
        let elapsed = start.elapsed().as_secs_f64();
        let rate = if elapsed > 0.0 {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let r = (total_records as f64 / elapsed).round() as u64;
            r
        } else {
            0
        };

        eprintln!(
            "\n{} Stats — {} files, {} records, {} parse errors  ({}/s, {elapsed:.2}s)",
            color::green("✓"),
            color::green(HumanCount(total_files as u64)),
            color::green(HumanCount(total_records)),
            if total_errors > 0 {
                color::yellow(HumanCount(total_errors))
            } else {
                color::green(HumanCount(0))
            },
            color::green(HumanCount(rate)),
        );
    }
}

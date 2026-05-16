pub mod frequency_bar;
pub mod latency_hist;

use crate::error::{FileError, Result};
use crate::features::{ChartEntry, ChartsConfig};

pub fn generate_charts(
    agg: &crate::features::TemplateAggregator,
    cfg: &ChartsConfig,
) -> Result<()> {
    let output_dir = std::path::Path::new(&cfg.output_dir);
    std::fs::create_dir_all(output_dir).map_err(|e| {
        crate::error::Error::File(FileError::CreateDirectoryFailed {
            path: output_dir.to_path_buf(),
            reason: e.to_string(),
        })
    })?;

    let entries: Vec<ChartEntry<'_>> = agg.iter_chart_entries().collect();

    if cfg.frequency_bar {
        let path = output_dir.join("top_n_frequency.svg");
        frequency_bar::draw_frequency_bar(&entries, cfg.top_n, &path)?;
    }

    if cfg.latency_hist {
        draw_all_latency_hists(&entries, cfg.top_n, output_dir)?;
    }

    Ok(())
}

fn draw_all_latency_hists(
    entries: &[ChartEntry<'_>],
    top_n: usize,
    output_dir: &std::path::Path,
) -> Result<()> {
    for entry in entries.iter().take(top_n) {
        let filename = format!("latency_histogram_{}.svg", sanitize_filename(entry.key));
        let path = output_dir.join(&filename);
        latency_hist::draw_latency_hist(entry.key, entry.histogram, &path)?;
    }
    Ok(())
}

fn sanitize_filename(key: &str) -> String {
    let sanitized: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    sanitized.chars().take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_ascii_alphanumeric() {
        assert_eq!(sanitize_filename("SELECT_count"), "SELECT_count");
    }

    #[test]
    fn test_sanitize_filename_spaces_and_special() {
        let result = sanitize_filename("SELECT count(*) FROM t");
        assert_eq!(result, "SELECT_count____FROM_t");
    }

    #[test]
    fn test_sanitize_filename_truncate_80() {
        let long_key = "a".repeat(100);
        let result = sanitize_filename(&long_key);
        assert_eq!(result.len(), 80);
    }

    #[test]
    fn test_sanitize_filename_non_ascii() {
        let result = sanitize_filename("查询_SELECT");
        // 中文字符替换为 _
        assert!(result.starts_with("__"));
        assert!(result.contains("SELECT"));
    }

    #[test]
    fn test_sanitize_filename_hyphen_preserved() {
        assert_eq!(sanitize_filename("key-name"), "key-name");
    }
}

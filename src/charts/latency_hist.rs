use crate::error::{Error, FileError};
use plotters::prelude::*;

const CHART_W: u32 = 1200;
const CHART_H: u32 = 600;
const STEELBLUE: RGBColor = RGBColor(70, 130, 180);

pub fn draw_latency_hist(
    key: &str,
    histogram: &hdrhistogram::Histogram<u64>,
    output_path: &std::path::Path,
) -> crate::error::Result<()> {
    let buckets = extract_buckets(histogram);
    if buckets.is_empty() {
        return Ok(());
    }

    let min_val = buckets.first().map_or(1, |(v, _)| (*v).max(1));
    let mut max_val = buckets.last().map_or(1, |(v, _)| *v);
    if max_val <= min_val {
        max_val = min_val + 1;
    }
    let max_count = buckets.iter().map(|(_, c)| *c).max().unwrap_or(1);

    draw_buckets(output_path, key, &buckets, min_val, max_val, max_count)
}

fn extract_buckets(histogram: &hdrhistogram::Histogram<u64>) -> Vec<(u64, u64)> {
    histogram
        .iter_recorded()
        .map(|v| (v.value_iterated_to(), v.count_at_value()))
        .collect()
}

fn draw_buckets(
    output_path: &std::path::Path,
    key: &str,
    buckets: &[(u64, u64)],
    min_val: u64,
    max_val: u64,
    max_count: u64,
) -> crate::error::Result<()> {
    let root = SVGBackend::new(output_path, (CHART_W, CHART_H)).into_drawing_area();
    root.fill(&WHITE)
        .map_err(|e| to_write_err(output_path, &e))?;

    let title: String = key.chars().take(60).collect();
    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("Latency: {title} (µs, log scale)"),
            ("sans-serif", 18),
        )
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d((min_val..max_val).log_scale(), 0u64..max_count)
        .map_err(|e| to_write_err(output_path, &e))?;

    chart
        .configure_mesh()
        .x_desc("Latency (µs)")
        .y_desc("Count")
        .draw()
        .map_err(|e| to_write_err(output_path, &e))?;

    chart
        .draw_series(buckets.windows(2).map(|pair| {
            let (left, _) = pair[0];
            let (right, count) = pair[1];
            Rectangle::new([(left, 0u64), (right, count)], STEELBLUE.filled())
        }))
        .map_err(|e| to_write_err(output_path, &e))?;

    root.present().map_err(|e| to_write_err(output_path, &e))?;
    Ok(())
}

fn to_write_err(path: &std::path::Path, e: &dyn std::error::Error) -> Error {
    Error::File(FileError::WriteFailed {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hdrhistogram::Histogram;
    use tempfile::TempDir;

    #[test]
    fn test_draw_latency_hist_empty_histogram() {
        let tmp = TempDir::new().unwrap();
        let output_path = tmp.path().join("empty.svg");
        let histogram: Histogram<u64> = Histogram::new_with_bounds(1, 60_000_000, 2).unwrap();

        let result = draw_latency_hist("SELECT 1", &histogram, &output_path);

        assert!(result.is_ok());
        assert!(!output_path.exists()); // 空 histogram 不创建文件
    }

    #[test]
    fn test_draw_latency_hist_creates_nonempty_svg() {
        let tmp = TempDir::new().unwrap();
        let output_path = tmp.path().join("latency.svg");
        let mut histogram: Histogram<u64> = Histogram::new_with_bounds(1, 60_000_000, 2).unwrap();
        histogram.record(100).unwrap();
        histogram.record(500).unwrap();
        histogram.record(1000).unwrap();
        histogram.record(5000).unwrap();

        let result = draw_latency_hist("SELECT 1", &histogram, &output_path);

        assert!(result.is_ok());
        assert!(output_path.exists());
        assert!(std::fs::metadata(&output_path).unwrap().len() > 0);
    }

    #[test]
    fn test_draw_latency_hist_single_bucket() {
        let tmp = TempDir::new().unwrap();
        let output_path = tmp.path().join("single.svg");
        let mut histogram: Histogram<u64> = Histogram::new_with_bounds(1, 60_000_000, 2).unwrap();
        // 同一值多次观测 → 单 bucket
        histogram.record(1000).unwrap();
        histogram.record(1000).unwrap();

        let result = draw_latency_hist("SELECT 1", &histogram, &output_path);

        assert!(result.is_ok());
        // 单 bucket 时 windows(2) 为空，图表无柱，但文件应已创建
    }

    #[test]
    fn test_extract_buckets_min_val_max_one() {
        let mut histogram: Histogram<u64> = Histogram::new_with_bounds(1, 60_000_000, 2).unwrap();
        histogram.record(1).unwrap();
        let buckets = extract_buckets(&histogram);
        assert!(!buckets.is_empty());
        let min_val = buckets.first().map_or(1, |(v, _)| (*v).max(1));
        assert!(min_val >= 1);
    }
}

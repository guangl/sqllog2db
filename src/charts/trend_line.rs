#![allow(dead_code)]

use crate::error::{Error, FileError};
use plotters::prelude::*;

const CHART_W: u32 = 1200;
const CHART_H: u32 = 600;
const LINE_COLOR: RGBColor = RGBColor(220, 50, 47);

pub fn draw_trend_line(
    hour_counts: &[(&str, u64)],
    output_path: &std::path::Path,
) -> crate::error::Result<()> {
    if hour_counts.is_empty() {
        return Ok(());
    }
    let labels = build_x_labels(hour_counts);
    let counts: Vec<u64> = hour_counts.iter().map(|(_, c)| *c).collect();
    let max_count = counts.iter().copied().max().unwrap_or(1);
    let n = counts.len();

    let root = SVGBackend::new(output_path, (CHART_W, CHART_H)).into_drawing_area();
    root.fill(&WHITE)
        .map_err(|e| to_write_err(output_path, e))?;

    draw_chart(&root, &labels, &counts, max_count, n, output_path)
        .map_err(|e| box_err_to_write_err(output_path, &*e))?;

    root.present().map_err(|e| to_write_err(output_path, e))?;
    Ok(())
}

fn is_multi_day(hour_counts: &[(&str, u64)]) -> bool {
    match (hour_counts.first(), hour_counts.last()) {
        (Some((first, _)), Some((last, _))) => {
            first.len() >= 10 && last.len() >= 10 && first[..10] != last[..10]
        }
        _ => false,
    }
}

fn format_bucket_label(bucket: &str, multi_day: bool) -> String {
    if multi_day && bucket.len() >= 13 {
        format!("{} {:}:00", &bucket[5..10], &bucket[11..13])
    } else if !multi_day && bucket.len() >= 13 {
        format!("{}:00", &bucket[11..13])
    } else {
        bucket.to_string()
    }
}

fn build_x_labels(hour_counts: &[(&str, u64)]) -> Vec<String> {
    let multi_day = is_multi_day(hour_counts);
    hour_counts
        .iter()
        .map(|(bucket, _)| format_bucket_label(bucket, multi_day))
        .collect()
}

fn draw_chart<DB: DrawingBackend>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    labels: &[String],
    counts: &[u64],
    max_count: u64,
    n: usize,
    output_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error + 'static>>
where
    DB::ErrorType: 'static,
{
    let mut chart = ChartBuilder::on(root)
        .caption("SQL Execution Trend by Hour", ("sans-serif", 20))
        .margin(20)
        .x_label_area_size(60)
        .y_label_area_size(80)
        .build_cartesian_2d((0..n).into_segmented(), 0u64..(max_count * 11 / 10 + 1))?;

    let labels_clone = labels.to_vec();
    chart
        .configure_mesh()
        .x_label_formatter(&|v: &SegmentValue<usize>| match v {
            SegmentValue::CenterOf(i) | SegmentValue::Exact(i) => {
                labels_clone.get(*i).cloned().unwrap_or_default()
            }
            SegmentValue::Last => String::new(),
        })
        .x_label_style(
            ("sans-serif", 11)
                .into_font()
                .transform(FontTransform::Rotate90),
        )
        .x_desc("Hour")
        .y_desc("SQL Count")
        .draw()?;

    chart.draw_series(LineSeries::new(
        counts
            .iter()
            .enumerate()
            .map(|(i, &c)| (SegmentValue::CenterOf(i), c)),
        LINE_COLOR.stroke_width(2),
    ))?;

    chart.draw_series(
        counts
            .iter()
            .enumerate()
            .map(|(i, &c)| Circle::new((SegmentValue::CenterOf(i), c), 4, LINE_COLOR.filled())),
    )?;

    let _ = output_path;
    Ok(())
}

fn to_write_err<E: std::error::Error>(path: &std::path::Path, e: E) -> Error {
    Error::File(FileError::WriteFailed {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

fn box_err_to_write_err(path: &std::path::Path, e: &dyn std::error::Error) -> Error {
    Error::File(FileError::WriteFailed {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_trend_line_empty_returns_ok() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("trend.svg");
        let result = draw_trend_line(&[], &path);
        assert!(result.is_ok());
        assert!(!path.exists()); // 空输入不创建文件
    }

    #[test]
    fn test_draw_trend_line_single_hour() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("trend.svg");
        let data = vec![("2025-01-15 10", 42u64)];
        let result = draw_trend_line(&data, &path);
        assert!(result.is_ok());
        assert!(path.exists());
        assert!(std::fs::metadata(&path).unwrap().len() > 0);
    }

    #[test]
    fn test_draw_trend_line_multi_hour() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("trend.svg");
        let data = vec![
            ("2025-01-15 09", 100u64),
            ("2025-01-15 10", 200u64),
            ("2025-01-15 11", 150u64),
        ];
        let result = draw_trend_line(&data, &path);
        assert!(result.is_ok());
        assert!(path.exists());
    }

    #[test]
    fn test_build_x_labels_single_day() {
        let data = vec![("2025-01-15 10", 5u64), ("2025-01-15 11", 3u64)];
        let labels = build_x_labels(&data);
        assert_eq!(labels[0], "10:00");
        assert_eq!(labels[1], "11:00");
    }

    #[test]
    fn test_build_x_labels_multi_day() {
        let data = vec![("2025-01-15 23", 5u64), ("2025-01-16 00", 3u64)];
        let labels = build_x_labels(&data);
        assert!(labels[0].contains("01-15"), "label: {}", labels[0]);
        assert!(labels[1].contains("01-16"), "label: {}", labels[1]);
    }

    #[test]
    fn test_is_multi_day_false() {
        let data = vec![("2025-01-15 10", 1u64), ("2025-01-15 22", 2u64)];
        assert!(!is_multi_day(&data));
    }

    #[test]
    fn test_is_multi_day_true() {
        let data = vec![("2025-01-15 23", 1u64), ("2025-01-16 00", 2u64)];
        assert!(is_multi_day(&data));
    }
}

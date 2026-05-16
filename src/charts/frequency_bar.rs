use crate::error::{Error, FileError};
use crate::features::ChartEntry;
use plotters::prelude::*;

const CHART_W: u32 = 1200;
const CHART_H: u32 = 600;
const STEELBLUE: RGBColor = RGBColor(70, 130, 180);
const MAX_LABEL_CHARS: usize = 40;

pub fn draw_frequency_bar(
    entries: &[ChartEntry<'_>],
    top_n: usize,
    output_path: &std::path::Path,
) -> crate::error::Result<()> {
    let data: Vec<(String, u64)> = entries
        .iter()
        .take(top_n)
        .map(|e| (super::truncate_label(e.key, MAX_LABEL_CHARS), e.count))
        .collect();

    if data.is_empty() {
        return Ok(());
    }

    let max_count = data.iter().map(|(_, c)| *c).max().unwrap_or(1);
    let n = data.len();

    let root = SVGBackend::new(output_path, (CHART_W, CHART_H)).into_drawing_area();
    root.fill(&WHITE)
        .map_err(|e| to_write_err(output_path, e))?;

    build_chart(&root, &data, max_count, n).map_err(|e| box_err_to_write_err(output_path, &*e))?;

    root.present().map_err(|e| to_write_err(output_path, e))?;
    Ok(())
}

fn build_chart<DB: DrawingBackend>(
    root: &DrawingArea<DB, plotters::coord::Shift>,
    data: &[(String, u64)],
    max_count: u64,
    n: usize,
) -> Result<(), Box<dyn std::error::Error + 'static>>
where
    DB::ErrorType: 'static,
{
    let labels: Vec<String> = data.iter().map(|(k, _)| k.clone()).collect();

    let mut chart = ChartBuilder::on(root)
        .caption(
            format!("Top {n} SQL Templates by Frequency"),
            ("sans-serif", 20),
        )
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(200)
        .build_cartesian_2d(0u64..max_count, (0..n).into_segmented())?;

    chart
        .configure_mesh()
        .y_label_formatter(&|v: &SegmentValue<usize>| match v {
            SegmentValue::CenterOf(i) | SegmentValue::Exact(i) => {
                labels.get(*i).cloned().unwrap_or_default()
            }
            SegmentValue::Last => String::new(),
        })
        .x_desc("Execution Count")
        .draw()?;

    chart.draw_series(
        Histogram::horizontal(&chart)
            .style(STEELBLUE.filled())
            .margin(5)
            .data(data.iter().enumerate().map(|(i, (_, count))| (i, *count))),
    )?;

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
    use super::super::truncate_label;
    use super::*;

    #[test]
    fn test_truncate_label_short() {
        assert_eq!(truncate_label("SELECT 1", 40), "SELECT 1");
    }

    #[test]
    fn test_truncate_label_exact_40() {
        let s = "a".repeat(40);
        assert_eq!(truncate_label(&s, 40), s);
    }

    #[test]
    fn test_truncate_label_41_chars() {
        let s = "a".repeat(41);
        let result = truncate_label(&s, 40);
        let char_count = result.chars().count();
        assert_eq!(char_count, 40);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_truncate_label_unicode() {
        let s = "中文".repeat(25); // 50 chars
        let result = truncate_label(&s, 40);
        assert!(result.chars().count() <= 40);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_draw_frequency_bar_creates_nonempty_svg() {
        use crate::features::TemplateAggregator;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let output_path = tmp.path().join("test.svg");

        let mut agg = TemplateAggregator::new();
        agg.observe("SELECT 1", 100, "2025-01-15 10:00:00", "");
        agg.observe("SELECT 2", 200, "2025-01-15 10:00:01", "");
        let entries: Vec<_> = agg.iter_chart_entries().collect();

        draw_frequency_bar(&entries, 10, &output_path).unwrap();

        assert!(output_path.exists());
        assert!(std::fs::metadata(&output_path).unwrap().len() > 0);
    }
}

use plotters::prelude::*;

const MAX_LABEL_CHARS: usize = 20;
const OTHERS_COLOR: RGBColor = RGBColor(150, 150, 150);

struct PieSlice {
    label: String,
    count: u64,
    color: RGBColor,
}

fn truncate_label(name: &str, max_chars: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max_chars {
        name.to_string()
    } else {
        let truncated: String = chars[..max_chars - 1].iter().collect();
        format!("{truncated}…")
    }
}

#[allow(clippy::many_single_char_names)]
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> RGBColor {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    RGBColor(
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

#[allow(clippy::cast_precision_loss)]
fn make_color(index: usize, total: usize) -> RGBColor {
    let hue = (index * 360 / total.max(1)) as f64;
    hsl_to_rgb(hue, 0.65, 0.55)
}

fn prepare_slices(user_counts: &[(&str, u64)], top_n: usize) -> Vec<PieSlice> {
    let top: Vec<_> = user_counts.iter().take(top_n).collect();
    let others_sum: u64 = user_counts.iter().skip(top_n).map(|(_, c)| c).sum();

    let total_named = top.len();
    let mut slices: Vec<PieSlice> = top
        .iter()
        .enumerate()
        .map(|(i, (name, count))| PieSlice {
            label: truncate_label(name, MAX_LABEL_CHARS),
            count: *count,
            color: make_color(i, total_named),
        })
        .collect();

    if others_sum > 0 {
        slices.push(PieSlice {
            label: "Others".to_string(),
            count: others_sum,
            color: OTHERS_COLOR,
        });
    }
    slices
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn sector_points(cx: f64, cy: f64, r: f64, start_rad: f64, end_rad: f64) -> Vec<(i32, i32)> {
    let mut pts = vec![(cx as i32, cy as i32)];
    let arc_len = (end_rad - start_rad).abs();
    let steps = (arc_len * 30.0) as usize + 2;
    for i in 0..=steps {
        let angle = start_rad + (end_rad - start_rad) * i as f64 / steps as f64;
        #[allow(clippy::cast_possible_truncation)]
        let x = (cx + r * angle.cos()) as i32;
        #[allow(clippy::cast_possible_truncation)]
        let y = (cy + r * angle.sin()) as i32;
        pts.push((x, y));
    }
    pts
}

fn to_write_err(path: &std::path::Path, e: &dyn std::error::Error) -> crate::error::Error {
    crate::error::Error::File(crate::error::FileError::WriteFailed {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

fn draw_legend(
    root: &DrawingArea<SVGBackend<'_>, plotters::coord::Shift>,
    slices: &[PieSlice],
    total: u64,
    output_path: &std::path::Path,
) -> crate::error::Result<()> {
    let legend_x = 580_i32;
    let legend_start_y = 60_i32;
    let row_h = 25_i32;

    for (i, slice) in slices.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let y = legend_start_y + i as i32 * row_h;
        root.draw(&Rectangle::new(
            [(legend_x, y), (legend_x + 16, y + 16)],
            slice.color.filled(),
        ))
        .map_err(|e| to_write_err(output_path, &e))?;
        #[allow(clippy::cast_precision_loss)]
        let pct = slice.count as f64 / total as f64 * 100.0;
        let label_text = format!("{} ({:.1}%)", slice.label, pct);
        root.draw(&Text::new(
            label_text,
            (legend_x + 22, y + 1),
            ("sans-serif", 13).into_font(),
        ))
        .map_err(|e| to_write_err(output_path, &e))?;
    }
    Ok(())
}

fn render_pie(
    slices: &[PieSlice],
    total: u64,
    output_path: &std::path::Path,
) -> crate::error::Result<()> {
    use std::f64::consts::PI;

    #[allow(clippy::cast_possible_truncation)]
    let required_h = 60 + (slices.len() as u32 + 1) * 25 + 20;
    let chart_h = required_h.max(600);
    let root = SVGBackend::new(output_path, (1000, chart_h)).into_drawing_area();
    root.fill(&WHITE)
        .map_err(|e| to_write_err(output_path, &e))?;

    root.draw(&Text::new(
        "SQL Executions by User",
        (500, 20),
        ("sans-serif", 20).into_font().color(&BLACK),
    ))
    .map_err(|e| to_write_err(output_path, &e))?;

    let cx = 280.0_f64;
    let cy = 300.0_f64;
    let r = 220.0_f64;
    let mut current_angle = -PI / 2.0;

    for slice in slices {
        #[allow(clippy::cast_precision_loss)]
        let fraction = slice.count as f64 / total as f64;
        let sweep = 2.0 * PI * fraction;
        let end_angle = current_angle + sweep;

        let pts = sector_points(cx, cy, r, current_angle, end_angle);
        root.draw(&Polygon::new(pts, slice.color.filled()))
            .map_err(|e| to_write_err(output_path, &e))?;

        current_angle = end_angle;
    }

    draw_legend(&root, slices, total, output_path)?;

    root.present().map_err(|e| to_write_err(output_path, &e))?;
    Ok(())
}

pub fn draw_user_pie(
    user_counts: &[(&str, u64)],
    top_n: usize,
    output_path: &std::path::Path,
) -> crate::error::Result<()> {
    if user_counts.is_empty() {
        return Ok(());
    }
    let slices = prepare_slices(user_counts, top_n);
    let total: u64 = slices.iter().map(|s| s.count).sum();
    if total == 0 {
        return Ok(());
    }
    render_pie(&slices, total, output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_draw_user_pie_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("pie.svg");
        let result = draw_user_pie(&[], 10, &path);
        assert!(result.is_ok());
        assert!(!path.exists());
    }

    #[test]
    fn test_draw_user_pie_single_user() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("pie.svg");
        let data = vec![("alice", 100_u64)];
        let result = draw_user_pie(&data, 10, &path);
        assert!(result.is_ok());
        assert!(path.exists());
        assert!(std::fs::metadata(&path).unwrap().len() > 0);
    }

    #[test]
    fn test_draw_user_pie_multiple_users() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("pie.svg");
        let data = vec![("alice", 300_u64), ("bob", 200_u64), ("carol", 100_u64)];
        let result = draw_user_pie(&data, 10, &path);
        assert!(result.is_ok());
        assert!(path.exists());
    }

    #[test]
    fn test_prepare_slices_within_top_n() {
        let data = vec![("alice", 300_u64), ("bob", 100_u64)];
        let slices = prepare_slices(&data, 5);
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].label, "alice");
        assert_eq!(slices[1].label, "bob");
    }

    #[test]
    fn test_prepare_slices_others_aggregation() {
        let data = vec![
            ("alice", 300_u64),
            ("bob", 200_u64),
            ("carol", 100_u64),
            ("dave", 50_u64),
        ];
        let slices = prepare_slices(&data, 2);
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0].label, "alice");
        assert_eq!(slices[1].label, "bob");
        assert_eq!(slices[2].label, "Others");
        assert_eq!(slices[2].count, 150);
    }

    #[test]
    fn test_truncate_label_long() {
        let long_name = "a".repeat(25);
        let result = truncate_label(&long_name, MAX_LABEL_CHARS);
        assert!(result.chars().count() <= MAX_LABEL_CHARS);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_hsl_to_rgb_red_hue() {
        let c = hsl_to_rgb(0.0, 0.65, 0.55);
        assert!(c.0 > 150, "R channel should be high for hue=0: {}", c.0);
    }
}

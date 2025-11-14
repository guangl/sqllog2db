//! Scatter feature
//!
//! - `read_stats_from_sqlite` (feature = "sqlite") reads (ts, body) from a table
//!   and buckets counts by SQL type detected from `body`.
//! - `scatter_to_svg` (feature = "scatter") draws a scatter plot using plotly.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::Path;

#[cfg(feature = "sqlite")]
use rusqlite::Connection;

#[cfg(feature = "scatter")]
use plotly::common::Mode;
#[cfg(feature = "scatter")]
use plotly::{Plot, Scatter};

/// SQL 类型枚举（按问题约定的前缀判断）
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SqlType {
    INS,
    DEL,
    UPD,
    SEL,
    DDL,
}

impl SqlType {
    /// 从 SQL body 前缀判断类型
    pub fn from_body(body: &str) -> Option<Self> {
        let body = body.trim();
        if body.starts_with("[INS]") {
            Some(Self::INS)
        } else if body.starts_with("[DEL]") {
            Some(Self::DEL)
        } else if body.starts_with("[UPD]") {
            Some(Self::UPD)
        } else if body.starts_with("[SEL]") {
            Some(Self::SEL)
        } else if body.starts_with("[DDL]") {
            Some(Self::DDL)
        } else {
            None
        }
    }
}

pub type ScatterStats = BTreeMap<SqlType, Vec<(i64, f64)>>;

/// 从 sqlite 读取 (ts INTEGER, body TEXT, exec_time_ms REAL) 并收集每个语句的数据
#[cfg(feature = "sqlite")]
pub fn read_stats_from_sqlite<P: AsRef<Path>>(
    sqlite_path: P,
    table: &str,
) -> Result<ScatterStats, Box<dyn Error>> {
    let conn = Connection::open(sqlite_path)?;
    let sql = format!("SELECT timestamp, body, exec_time_ms FROM {}", table);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        // timestamp column in DB is stored as TEXT like "YYYY-MM-DD HH:MM:SS.sss"
        // Try integer first, then fallback to string parse.
        let ts_res: Result<i64, _> = row.get(0);
        let ts: i64 = match ts_res {
            Ok(v) => v,
            Err(_) => {
                let s: String = row.get(0)?;
                // try parse using chrono if available
                match chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S%.f") {
                    Ok(dt) => dt.timestamp(),
                    Err(_) => {
                        // try trim to seconds
                        if s.len() >= 19 {
                            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(
                                &s[0..19],
                                "%Y-%m-%d %H:%M:%S",
                            ) {
                                dt.timestamp()
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    }
                }
            }
        };
        let body: String = row.get(1)?;
        let exec_time: Option<f64> = row.get(2)?;
        Ok((ts, body, exec_time))
    })?;

    let mut stats: ScatterStats = BTreeMap::new();
    for r in rows {
        let (ts, body, exec_time) = r?;
        if let Some(exec_time) = exec_time {
            if let Some(sqlt) = SqlType::from_body(&body) {
                stats.entry(sqlt).or_default().push((ts, exec_time));
            }
        }
    }
    Ok(stats)
}

/// 使用 plotly 将统计绘制为 SVG 散点图
#[cfg(feature = "scatter")]
pub fn scatter_to_svg<P: AsRef<Path>>(
    stats: &ScatterStats,
    svg_path: P,
) -> Result<(), Box<dyn Error>> {
    let mut plot = Plot::new();

    let colors = vec![
        "rgb(226, 74, 51)",   // INS - red
        "rgb(52, 138, 189)",  // DEL - blue
        "rgb(152, 142, 213)", // UPD - purple
        "rgb(119, 119, 119)", // SEL - gray
        "rgb(251, 193, 94)",  // DDL - orange
    ];

    let types = vec![
        SqlType::INS,
        SqlType::DEL,
        SqlType::UPD,
        SqlType::SEL,
        SqlType::DDL,
    ];

    for (i, sqlt) in types.iter().enumerate() {
        if let Some(data) = stats.get(sqlt) {
            let x_values: Vec<String> = data
                .iter()
                .map(|&(ts, _)| {
                    if let Some(dt) = chrono::NaiveDateTime::from_timestamp_opt(ts, 0) {
                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                    } else {
                        ts.to_string()
                    }
                })
                .collect();
            let y_values: Vec<f64> = data.iter().map(|&(_, exec_time)| exec_time).collect();

            let trace = Scatter::new(x_values, y_values)
                .name(format!("{:?}", sqlt))
                .mode(Mode::Markers)
                .marker(plotly::common::Marker::new().color(colors[i]).size(6));

            plot.add_trace(trace);
        }
    }

    plot.set_layout(
        plotly::Layout::new()
            .title(plotly::common::Title::with_text("SQL Scatter Plot"))
            .x_axis(
                plotly::layout::Axis::new()
                    .title("Time")
                    .type_(plotly::layout::AxisType::Date),
            )
            .y_axis(plotly::layout::Axis::new().title("Execution Time (ms)"))
            .show_legend(true),
    );

    plot.write_html(svg_path);
    Ok(())
}

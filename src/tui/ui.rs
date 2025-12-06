#[cfg(feature = "tui")]
use crate::tui::app::TuiApp;
#[cfg(feature = "tui")]
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
#[cfg(feature = "tui")]
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph},
};
#[cfg(feature = "tui")]
use std::io;
#[cfg(feature = "tui")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "tui")]
use std::time::Duration;

/// 初始化终端
#[cfg(feature = "tui")]
pub fn init_terminal() -> io::Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

/// 恢复终端
#[cfg(feature = "tui")]
pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// 处理用户输入
#[cfg(feature = "tui")]
pub fn handle_input() -> io::Result<bool> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// 绘制 UI
#[cfg(feature = "tui")]
pub fn draw_ui(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(f.area());

    // 标题
    let title = Paragraph::new("SQL Log Export Monitor")
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // 进度条
    let progress = Gauge::default()
        .block(Block::default().title("Progress").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green).bold())
        .percent(app.progress_percent() as u16)
        .label(format!(
            "{}/{} files",
            app.current_file_index, app.total_files
        ));
    f.render_widget(progress, chunks[1]);

    // 当前文件信息
    let file_info = Paragraph::new(format!(
        "Current File: {}\nExporter: {}",
        app.current_file_name, app.exporter_name
    ))
    .block(Block::default().title("File Info").borders(Borders::ALL))
    .style(Style::default().fg(Color::White));
    f.render_widget(file_info, chunks[2]);

    // 统计信息
    let elapsed = app.elapsed_secs();
    let throughput = app.throughput();
    let stats = Paragraph::new(format!(
        "Records: {}\nErrors: {}\nElapsed: {:.0}s\nThroughput: {:.0} rec/s",
        app.exported_records, app.error_records, elapsed as f64, throughput
    ))
    .block(Block::default().title("Statistics").borders(Borders::ALL))
    .style(Style::default().fg(Color::Yellow));
    f.render_widget(stats, chunks[3]);
}

/// 运行 TUI
#[cfg(feature = "tui")]
pub async fn run_tui(app_state: Arc<Mutex<TuiApp>>) -> io::Result<()> {
    let mut terminal = init_terminal()?;
    terminal.clear()?;

    let result = loop {
        let app = app_state.lock().unwrap().clone();

        terminal.draw(|f| draw_ui(f, &app))?;

        if !handle_input()? || app.is_finished {
            break Ok(());
        }
    };

    let mut terminal = init_terminal()?;
    restore_terminal(&mut terminal)?;

    result
}

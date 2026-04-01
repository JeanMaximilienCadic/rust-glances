//! Container logs viewer overlay.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap, block::BorderType},
    Frame,
};

use crate::app::App;

/// Render the container logs viewer overlay.
pub fn render_logs_dialog(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref logs) = app.container_logs else { return };

    let dialog_area = centered_rect(90, 85, area);
    frame.render_widget(Clear, dialog_area);

    let title = format!(" Logs: {} ({}) ", logs.container_name, logs.container_id);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(Style::default().fg(Color::Rgb(130, 170, 255)));

    let visible_height = chunks[0].height.saturating_sub(2) as usize;
    let start = logs.scroll.saturating_sub(visible_height / 2);
    let lines: Vec<Line> = logs.lines.iter().skip(start).take(visible_height).map(|line| {
        // Color-code log lines
        let color = if line.contains("ERROR") || line.contains("error") || line.contains("CRITICAL") {
            Color::Rgb(255, 80, 80)
        } else if line.contains("WARNING") || line.contains("warn") || line.contains("WARN") {
            Color::Rgb(255, 180, 50)
        } else if line.contains("INFO") || line.contains("info") {
            Color::Rgb(80, 220, 120)
        } else if line.contains("DEBUG") || line.contains("debug") {
            Color::DarkGray
        } else {
            Color::White
        };
        Line::from(Span::styled(line.clone(), Style::default().fg(color)))
    }).collect();

    frame.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), chunks[0]);

    // Help
    let help = Line::from(vec![
        Span::styled(" j/k", Style::default().fg(Color::Rgb(130, 170, 255))),
        Span::styled(":Scroll ", Style::default().fg(Color::DarkGray)),
        Span::styled("PgUp/PgDn", Style::default().fg(Color::Rgb(130, 170, 255))),
        Span::styled(":Page ", Style::default().fg(Color::DarkGray)),
        Span::styled("Home/End", Style::default().fg(Color::Rgb(130, 170, 255))),
        Span::styled(":Jump ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" [{}/{}]", logs.scroll + 1, logs.lines.len()), Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled("Esc", Style::default().fg(Color::Rgb(255, 80, 80))),
        Span::styled(":Close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

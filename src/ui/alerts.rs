//! Alert events panel — shows system warnings like high memory/CPU.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, block::BorderType},
    Frame,
};

use crate::app::{AlertLevel, App};

/// Render the alerts panel.
pub fn render_alerts_panel(frame: &mut Frame, area: Rect, app: &App) {
    if app.alerts.is_empty() {
        return;
    }

    let max_lines = area.height.saturating_sub(2) as usize;

    // Show most recent alerts, ongoing first
    let mut sorted = app.alerts.clone();
    sorted.sort_by(|a, b| b.ongoing.cmp(&a.ongoing).then(b.timestamp.cmp(&a.timestamp)));

    let lines: Vec<Line> = sorted.iter().take(max_lines).map(|alert| {
        let (icon, color) = match (&alert.level, alert.ongoing) {
            (AlertLevel::Critical, true) => ("●", Color::Rgb(255, 80, 80)),
            (AlertLevel::Warning, true) => ("●", Color::Rgb(255, 180, 50)),
            (_, false) => ("○", Color::DarkGray),
        };
        let status = if alert.ongoing { "(ongoing)" } else { "(resolved)" };

        Line::from(vec![
            Span::styled(format!(" {} ", icon), Style::default().fg(color)),
            Span::styled(&alert.timestamp, Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(
                &alert.message,
                Style::default().fg(color).add_modifier(if alert.ongoing { Modifier::BOLD } else { Modifier::empty() }),
            ),
            Span::styled(format!(" {}", status), Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Alerts ")
        .border_style(Style::default().fg(Color::Rgb(255, 180, 50)));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

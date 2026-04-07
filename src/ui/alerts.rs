//! Alert events panel — shows system warnings with top contributing processes.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, block::BorderType},
    Frame,
};

use crate::app::{AlertLevel, App};

/// Render the alerts panel with process details.
pub fn render_alerts_panel(frame: &mut Frame, area: Rect, app: &App) {
    let max_lines = area.height.saturating_sub(2) as usize;
    let ongoing_count = app.alerts.iter().filter(|a| a.ongoing).count();

    // Show "No active alerts" when empty
    if app.alerts.is_empty() || ongoing_count == 0 {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Alerts ")
            .border_style(Style::default().fg(Color::Rgb(80, 100, 80)));

        let text = Line::from(Span::styled(
            " ✓ No active alerts",
            Style::default().fg(Color::Rgb(80, 180, 80)),
        ));

        frame.render_widget(Paragraph::new(text).block(block), area);
        return;
    }

    // Show ongoing alerts first, then most recent resolved
    let mut sorted = app.alerts.clone();
    sorted.sort_by(|a, b| b.ongoing.cmp(&a.ongoing).then(b.timestamp.cmp(&a.timestamp)));

    let mut lines: Vec<Line> = Vec::new();

    for alert in sorted.iter() {
        if lines.len() >= max_lines {
            break;
        }

        let (icon, color) = match (&alert.level, alert.ongoing) {
            (AlertLevel::Critical, true) => ("●", Color::Rgb(255, 80, 80)),
            (AlertLevel::Warning, true) => ("●", Color::Rgb(255, 180, 50)),
            (_, false) => ("○", Color::Rgb(80, 80, 100)),
        };

        // Build process list string
        let procs_str = if !alert.top_processes.is_empty() {
            let proc_parts: Vec<String> = alert.top_processes.iter()
                .map(|(name, val)| format!("{}({})", name, val))
                .collect();
            format!(" → {}", proc_parts.join(", "))
        } else {
            String::new()
        };

        let status = if alert.ongoing { "(ongoing)" } else { "(resolved)" };

        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", icon), Style::default().fg(color)),
            Span::styled(alert.timestamp.clone(), Style::default().fg(Color::Rgb(80, 80, 100))),
            Span::raw(" "),
            Span::styled(
                alert.message.clone(),
                Style::default().fg(color).add_modifier(if alert.ongoing { Modifier::BOLD } else { Modifier::empty() }),
            ),
            Span::styled(
                procs_str,
                Style::default().fg(if alert.ongoing { Color::White } else { Color::Rgb(80, 80, 100) }),
            ),
            Span::styled(format!(" {}", status), Style::default().fg(Color::Rgb(60, 60, 80))),
        ]));
    }

    let title = if ongoing_count > 0 {
        format!(" Alerts ({} active) ", ongoing_count)
    } else {
        " Alerts ".into()
    };

    let border_color = if ongoing_count > 0 {
        Color::Rgb(255, 180, 50)
    } else {
        Color::Rgb(80, 80, 120)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(Style::default().fg(border_color));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

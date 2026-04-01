//! Temperature/sensor panel rendering — modern compact style.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, block::BorderType},
    Frame,
};

use crate::app::App;

fn temp_gradient(temp: f32) -> Color {
    if temp >= 85.0 {
        Color::Rgb(255, 80, 80)
    } else if temp >= 70.0 {
        Color::Rgb(255, 180, 50)
    } else if temp >= 50.0 {
        Color::Rgb(100, 200, 255)
    } else {
        Color::Rgb(80, 220, 120)
    }
}

/// Render the temperature sensors panel.
pub fn render_temps_panel(frame: &mut Frame, area: Rect, app: &App) {
    if app.system_metrics.temperatures.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Sensors ")
            .border_style(Style::default().fg(Color::DarkGray));
        frame.render_widget(block, area);
        return;
    }

    let max_rows = (area.height as usize).saturating_sub(2);
    let mut lines: Vec<Line> = Vec::new();

    for (label, temp) in app.system_metrics.temperatures.iter().take(max_rows) {
        let short_label = if label.len() > 14 {
            format!("{}…", &label[..13])
        } else {
            format!("{:<14}", label)
        };

        lines.push(Line::from(vec![
            Span::styled(short_label, Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" {:5.1}°C", temp),
                Style::default().fg(temp_gradient(*temp)),
            ),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Sensors ")
        .border_style(Style::default().fg(Color::Rgb(80, 80, 120)));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

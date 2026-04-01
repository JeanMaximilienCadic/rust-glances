//! Footer bar rendering — modern minimal keybinding hints.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;

/// Render the footer bar with keyboard shortcuts.
pub fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let refresh_ms = app.refresh_rate.as_millis();
    let dim = Style::default().fg(Color::DarkGray);
    let key = Style::default().fg(Color::Rgb(130, 170, 255)).add_modifier(Modifier::BOLD);
    let danger = Style::default().fg(Color::Rgb(255, 80, 80)).add_modifier(Modifier::BOLD);

    let footer = Line::from(vec![
        Span::styled(" 1-6", key),
        Span::styled(":Views ", dim),
        Span::styled("?", key),
        Span::styled(":Help ", dim),
        Span::styled("F2-F8", key),
        Span::styled(":Sort ", dim),
        Span::styled("r", key),
        Span::styled(":Rev ", dim),
        Span::styled("a", key),
        Span::styled(":All ", dim),
        Span::styled("g", key),
        Span::styled(":Graphs ", dim),
        Span::styled("p", key),
        Span::styled(":Cores ", dim),
        Span::styled("d", key),
        Span::styled(":Docker ", dim),
        Span::styled("t", key),
        Span::styled(":Temps ", dim),
        Span::styled("+/-", key),
        Span::styled(format!(":{}ms ", refresh_ms), dim),
        Span::styled("Del", danger),
        Span::styled(":Kill ", dim),
        Span::styled("q", danger),
        Span::styled(":Quit", dim),
    ]);

    frame.render_widget(Paragraph::new(footer), area);
}

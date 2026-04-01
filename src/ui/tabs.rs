//! Tab bar rendering for view switching.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, ViewTab};

/// Render the tab bar.
pub fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let tabs = [
        (ViewTab::Overview, "Overview"),
        (ViewTab::Processes, "Processes"),
        (ViewTab::Network, "Network"),
        (ViewTab::Disks, "Disks"),
        (ViewTab::Docker, "Docker"),
        (ViewTab::Gpu, "GPU"),
    ];

    let mut spans = vec![Span::raw(" ")];

    for (i, (tab, label)) in tabs.iter().enumerate() {
        let is_active = app.active_tab == *tab;

        if i > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        }

        spans.push(Span::styled(
            format!("{}", i + 1),
            Style::default().fg(Color::Rgb(100, 100, 140)),
        ));
        spans.push(Span::raw(" "));

        if is_active {
            spans.push(Span::styled(
                format!(" {} ", label),
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Rgb(60, 60, 100))
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} ", label),
                Style::default().fg(Color::Gray),
            ));
        }
    }

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}

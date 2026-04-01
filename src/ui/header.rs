//! Header bar rendering — modern minimal style.

use chrono::Local;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;
use crate::types::GpuBackend;
use crate::utils::format_duration;

/// Render the header bar.
pub fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let uptime = format_duration(app.system_metrics.uptime);
    let now = Local::now();

    let gpu_info = if let Some(ref gm) = app.gpu_metrics {
        let label = match gm.backend {
            GpuBackend::Nvml => "CUDA",
            GpuBackend::Metal => "Metal",
            GpuBackend::None => "",
        };
        if !label.is_empty() {
            format!(" │ {} {}", label, gm.api_version)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mut header = Line::from(vec![
        Span::styled(
            " glances ",
            Style::default()
                .fg(Color::Rgb(130, 170, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled(
            &app.system_metrics.hostname,
            Style::default().fg(Color::Rgb(80, 220, 120)),
        ),
        Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled(
            &app.system_metrics.os_name,
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled(
            format!("up {}", uptime),
            Style::default().fg(Color::Rgb(255, 220, 100)),
        ),
        Span::styled(&gpu_info, Style::default().fg(Color::Rgb(100, 200, 255))),
    ]);

    // Battery info
    if let Some(pct) = app.system_metrics.battery_pct {
        let icon = if pct >= 80.0 { "█" } else if pct >= 50.0 { "▓" } else if pct >= 20.0 { "▒" } else { "░" };
        let color = if pct >= 50.0 {
            Color::Rgb(80, 220, 120)
        } else if pct >= 20.0 {
            Color::Rgb(255, 180, 50)
        } else {
            Color::Rgb(255, 80, 80)
        };
        let state_icon = if app.system_metrics.battery_state == "Charging" { "⚡" } else { "" };
        header.spans.push(Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 80))));
        header.spans.push(Span::styled(
            format!("{}{} {:.0}%{}", icon, icon, pct, state_icon),
            Style::default().fg(color),
        ));
    }

    header.spans.push(Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 80))));
    header.spans.push(Span::styled(
        now.format("%H:%M:%S").to_string(),
        Style::default().fg(Color::White),
    ));

    frame.render_widget(Paragraph::new(header), area);
}

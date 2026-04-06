//! History graph rendering — GPU utilization charts.

use ratatui::{
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, block::BorderType},
    Frame,
    layout::Rect,
};

use crate::app::App;

const BORDER_COLOR: Color = Color::Rgb(80, 80, 120);
const AXIS_COLOR: Color = Color::Rgb(60, 60, 80);

/// Render GPU utilization graph — line chart with area hints.
pub fn render_gpu_graphs(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref gpu_metrics) = app.gpu_metrics else { return };
    if gpu_metrics.gpus.is_empty() { return }

    let history = &app.history.gpu_util_history;

    let gpu_colors = [
        Color::Rgb(80, 220, 120),
        Color::Rgb(255, 180, 50),
        Color::Rgb(100, 200, 255),
        Color::Rgb(255, 100, 100),
    ];
    let data_vecs: Vec<Vec<(f64, f64)>> = history.iter()
        .map(|h| h.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect())
        .collect();

    let mut datasets = Vec::new();
    let mut title_spans = vec![Span::raw(" ")];

    for (i, data) in data_vecs.iter().enumerate().take(4) {
        let color = gpu_colors[i % gpu_colors.len()];
        let now = history.get(i).and_then(|h| h.last()).copied().unwrap_or(0.0);
        datasets.push(
            Dataset::default()
                .name(format!("GPU{} {:.0}%", i, now))
                .marker(symbols::Marker::HalfBlock)
                .graph_type(ratatui::widgets::GraphType::Line)
                .style(Style::default().fg(color))
                .data(data),
        );
        if i > 0 {
            title_spans.push(Span::styled(" · ", Style::default().fg(BORDER_COLOR)));
        }
        title_spans.push(Span::styled(
            format!("GPU{}", i),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
        title_spans.push(Span::styled(
            format!(" {:.0}%", now),
            Style::default().fg(color),
        ));
    }
    title_spans.push(Span::raw(" "));

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Line::from(title_spans))
                .border_style(Style::default().fg(BORDER_COLOR)),
        )
        .x_axis(Axis::default().bounds([0.0, 59.0]).labels::<Vec<Line>>(vec![]))
        .y_axis(
            Axis::default()
                .style(Style::default().fg(AXIS_COLOR))
                .bounds([0.0, 100.0])
                .labels(vec![
                    Line::from(Span::styled("0", Style::default().fg(AXIS_COLOR))),
                    Line::from(Span::styled("50", Style::default().fg(AXIS_COLOR))),
                    Line::from(Span::styled("100", Style::default().fg(AXIS_COLOR))),
                ]),
        );

    frame.render_widget(chart, area);
}

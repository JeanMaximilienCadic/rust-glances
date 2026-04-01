//! History graph rendering — smooth braille charts with modern styling.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, Sparkline, block::BorderType},
    Frame,
};

use crate::app::App;
use crate::types::GpuBackend;

/// Color palette for graphs.
const CPU_COLOR: Color = Color::Rgb(130, 170, 255);
const MEM_COLOR: Color = Color::Rgb(200, 130, 255);
const NET_RX_COLOR: Color = Color::Rgb(80, 220, 120);
const NET_TX_COLOR: Color = Color::Rgb(255, 100, 100);
const BORDER_COLOR: Color = Color::Rgb(80, 80, 120);
const AXIS_COLOR: Color = Color::Rgb(60, 60, 80);
const GPU_COLORS: [Color; 4] = [
    Color::Rgb(80, 220, 120),
    Color::Rgb(255, 180, 50),
    Color::Rgb(100, 200, 255),
    Color::Rgb(255, 100, 100),
];

/// Render CPU and memory history graph — dual line chart with sparkline mini-preview.
pub fn render_cpu_mem_graph(frame: &mut Frame, area: Rect, app: &App) {
    let cpu_data: Vec<(f64, f64)> = app.history.cpu_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v)).collect();
    let mem_data: Vec<(f64, f64)> = app.history.memory_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v)).collect();

    // Current values for legend
    let cpu_now = app.history.cpu_history.last().copied().unwrap_or(0.0);
    let mem_now = app.history.memory_history.last().copied().unwrap_or(0.0);

    let datasets = vec![
        Dataset::default()
            .name(format!("CPU {:.0}%", cpu_now))
            .marker(symbols::Marker::Braille)
            .graph_type(ratatui::widgets::GraphType::Line)
            .style(Style::default().fg(CPU_COLOR))
            .data(&cpu_data),
        Dataset::default()
            .name(format!("MEM {:.0}%", mem_now))
            .marker(symbols::Marker::Braille)
            .graph_type(ratatui::widgets::GraphType::Line)
            .style(Style::default().fg(MEM_COLOR))
            .data(&mem_data),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Line::from(vec![
                    Span::styled(" CPU", Style::default().fg(CPU_COLOR).add_modifier(Modifier::BOLD)),
                    Span::styled(" + ", Style::default().fg(BORDER_COLOR)),
                    Span::styled("MEM ", Style::default().fg(MEM_COLOR).add_modifier(Modifier::BOLD)),
                ]))
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

/// Render GPU utilization history graph.
pub fn render_gpu_graphs(frame: &mut Frame, area: Rect, app: &App) {
    let Some(ref gpu_metrics) = app.gpu_metrics else { return };
    if gpu_metrics.gpus.is_empty() { return }

    let is_metal = gpu_metrics.backend == GpuBackend::Metal;
    let history = if is_metal { &app.history.gpu_mem_history } else { &app.history.gpu_util_history };

    let data_vecs: Vec<Vec<(f64, f64)>> = history.iter()
        .map(|h| h.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect())
        .collect();

    let mut datasets = Vec::new();
    let mut title_spans = vec![Span::raw(" ")];

    for (i, data) in data_vecs.iter().enumerate().take(4) {
        let color = GPU_COLORS[i % GPU_COLORS.len()];
        let now = history.get(i).and_then(|h| h.last()).copied().unwrap_or(0.0);
        datasets.push(
            Dataset::default()
                .name(format!("GPU{} {:.0}%", i, now))
                .marker(symbols::Marker::Braille)
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
    }
    title_spans.push(Span::raw(" "));

    let label = if is_metal { "Memory" } else { "Utilization" };
    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Line::from(vec![
                    Span::styled(format!(" GPU {} ", label), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]))
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

/// Render a combined overview graph panel — CPU sparkline + MEM sparkline + Network sparkline.
pub fn render_sparkline_row(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    // CPU sparkline
    let cpu_data: Vec<u64> = app.history.cpu_history.iter()
        .map(|&v| v as u64).collect();
    let cpu_spark = Sparkline::default()
        .block(Block::default().title(" CPU ").borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_COLOR)))
        .data(&cpu_data)
        .max(100)
        .style(Style::default().fg(CPU_COLOR));
    frame.render_widget(cpu_spark, chunks[0]);

    // Memory sparkline
    let mem_data: Vec<u64> = app.history.memory_history.iter()
        .map(|&v| v as u64).collect();
    let mem_spark = Sparkline::default()
        .block(Block::default().title(" MEM ").borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_COLOR)))
        .data(&mem_data)
        .max(100)
        .style(Style::default().fg(MEM_COLOR));
    frame.render_widget(mem_spark, chunks[1]);

    // Network sparkline (Rx)
    let net_data: Vec<u64> = app.history.network_rx_history.iter()
        .map(|&v| (v * 100.0) as u64).collect(); // scale for visibility
    let max_net = net_data.iter().copied().max().unwrap_or(1).max(1);
    let net_spark = Sparkline::default()
        .block(Block::default().title(" NET ▼ ").borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_COLOR)))
        .data(&net_data)
        .max(max_net)
        .style(Style::default().fg(NET_RX_COLOR));
    frame.render_widget(net_spark, chunks[2]);
}

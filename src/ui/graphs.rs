//! History graph rendering — fancy area-fill charts with HalfBlock markers.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Chart, Dataset, Sparkline, block::BorderType,
        canvas::{Canvas, Points},
    },
    Frame,
};

use crate::app::App;

const BORDER_COLOR: Color = Color::Rgb(80, 80, 120);
const AXIS_COLOR: Color = Color::Rgb(60, 60, 80);

/// Color with RGB interpolation based on value.
fn heat_color(pct: f64) -> Color {
    if pct >= 90.0 {
        Color::Rgb(255, 60, 60)
    } else if pct >= 75.0 {
        let t = (pct - 75.0) / 15.0;
        Color::Rgb(255, (180.0 - t * 120.0) as u8, (50.0 - t * 50.0) as u8)
    } else if pct >= 50.0 {
        let t = (pct - 50.0) / 25.0;
        Color::Rgb((100.0 + t * 155.0) as u8, (200.0 - t * 20.0) as u8, (255.0 - t * 205.0) as u8)
    } else if pct >= 25.0 {
        let t = (pct - 25.0) / 25.0;
        Color::Rgb((80.0 + t * 20.0) as u8, (220.0 - t * 20.0) as u8, (120.0 + t * 135.0) as u8)
    } else {
        Color::Rgb(60, 180, 100)
    }
}

/// Render a fancy area-fill CPU+Memory chart using Canvas for the fill + Line overlay.
pub fn render_cpu_mem_graph(frame: &mut Frame, area: Rect, app: &App) {
    let cpu_now = app.history.cpu_history.last().copied().unwrap_or(0.0);
    let mem_now = app.history.memory_history.last().copied().unwrap_or(0.0);

    // Use Canvas for area fill effect
    let cpu_history = app.history.cpu_history.clone();
    let mem_history = app.history.memory_history.clone();

    let canvas = Canvas::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Line::from(vec![
                    Span::styled(" CPU", Style::default().fg(Color::Rgb(130, 170, 255)).add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" {:.0}%", cpu_now), Style::default().fg(Color::Rgb(130, 170, 255))),
                    Span::styled(" │ ", Style::default().fg(BORDER_COLOR)),
                    Span::styled("MEM", Style::default().fg(Color::Rgb(200, 130, 255)).add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" {:.0}% ", mem_now), Style::default().fg(Color::Rgb(200, 130, 255))),
                ]))
                .border_style(Style::default().fg(BORDER_COLOR)),
        )
        .x_bounds([0.0, 59.0])
        .y_bounds([0.0, 100.0])
        .paint(move |ctx| {
            // Draw CPU area fill (columns of dots from 0 to value)
            for (i, &val) in cpu_history.iter().enumerate() {
                let x = i as f64;
                // Draw filled area with sparse dots
                let steps = (val / 4.0).ceil() as usize;
                for s in 0..steps {
                    let y = s as f64 * 4.0;
                    if y <= val {
                        ctx.draw(&Points {
                            coords: &[(x, y)],
                            color: Color::Rgb(80, 110, 180),
                        });
                    }
                }
                // Draw the line at top
                ctx.draw(&Points {
                    coords: &[(x, val)],
                    color: Color::Rgb(130, 170, 255),
                });
            }

            // Draw MEM line on top
            for (i, &val) in mem_history.iter().enumerate() {
                let x = i as f64;
                ctx.draw(&Points {
                    coords: &[(x, val)],
                    color: Color::Rgb(200, 130, 255),
                });
                // Lighter fill for memory
                let steps = (val / 6.0).ceil() as usize;
                for s in 0..steps {
                    let y = s as f64 * 6.0;
                    if y <= val {
                        ctx.draw(&Points {
                            coords: &[(x, y)],
                            color: Color::Rgb(120, 80, 160),
                        });
                    }
                }
            }
        });

    frame.render_widget(canvas, area);
}

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

/// Render a row of mini sparklines — CPU, Memory, Network.
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
    let cpu_data: Vec<u64> = app.history.cpu_history.iter().map(|&v| v as u64).collect();
    let cpu_now = app.history.cpu_history.last().copied().unwrap_or(0.0);
    let cpu_spark = Sparkline::default()
        .block(
            Block::default()
                .title(Line::from(vec![
                    Span::styled(" CPU ", Style::default().fg(Color::Rgb(130, 170, 255)).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{:.0}% ", cpu_now), Style::default().fg(heat_color(cpu_now))),
                ]))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_COLOR)),
        )
        .data(&cpu_data)
        .max(100)
        .style(Style::default().fg(Color::Rgb(130, 170, 255)));
    frame.render_widget(cpu_spark, chunks[0]);

    // Memory sparkline
    let mem_data: Vec<u64> = app.history.memory_history.iter().map(|&v| v as u64).collect();
    let mem_now = app.history.memory_history.last().copied().unwrap_or(0.0);
    let mem_spark = Sparkline::default()
        .block(
            Block::default()
                .title(Line::from(vec![
                    Span::styled(" MEM ", Style::default().fg(Color::Rgb(200, 130, 255)).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{:.0}% ", mem_now), Style::default().fg(heat_color(mem_now))),
                ]))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_COLOR)),
        )
        .data(&mem_data)
        .max(100)
        .style(Style::default().fg(Color::Rgb(200, 130, 255)));
    frame.render_widget(mem_spark, chunks[1]);

    // Network sparkline (Rx)
    let net_data: Vec<u64> = app.history.network_rx_history.iter()
        .map(|&v| (v * 100.0) as u64).collect();
    let max_net = net_data.iter().copied().max().unwrap_or(1).max(1);
    let net_now = app.history.network_rx_history.last().copied().unwrap_or(0.0);
    let net_label = if net_now >= 1.0 { format!("{:.1}MB/s ", net_now) }
        else { format!("{:.0}KB/s ", net_now * 1024.0) };
    let net_spark = Sparkline::default()
        .block(
            Block::default()
                .title(Line::from(vec![
                    Span::styled(" NET▼ ", Style::default().fg(Color::Rgb(80, 220, 120)).add_modifier(Modifier::BOLD)),
                    Span::styled(net_label, Style::default().fg(Color::Rgb(80, 220, 120))),
                ]))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_COLOR)),
        )
        .data(&net_data)
        .max(max_net)
        .style(Style::default().fg(Color::Rgb(80, 220, 120)));
    frame.render_widget(net_spark, chunks[2]);
}

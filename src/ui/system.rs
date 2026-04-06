//! System panel rendering — modern inline bars for CPU, MEM, SWAP, LOAD.

use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, block::BorderType},
    Frame,
};

use crate::app::App;

// Layout constants for column alignment
const LABEL_W: usize = 5;  // "CPU  ", "MEM  ", etc.
const PCT_W: usize = 7;    // " 15.3%"

fn gradient_color_for(pct: f64) -> Color {
    if pct >= 90.0 { Color::Rgb(255, 80, 80) }
    else if pct >= 70.0 { Color::Rgb(255, 180, 50) }
    else if pct >= 50.0 { Color::Rgb(100, 200, 255) }
    else { Color::Rgb(80, 220, 120) }
}

/// Build a gradient bar of exact `width` characters.
fn gradient_bar(pct: f64, width: usize) -> Vec<Span<'static>> {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    vec![
        Span::styled("▓".repeat(filled), Style::default().fg(gradient_color_for(pct))),
        Span::styled("░".repeat(empty), Style::default().fg(Color::Rgb(40, 40, 50))),
    ]
}

/// Compute bar width so all rows align: total width - label - pct - separator - 3 stat columns.
fn bar_width(area_width: u16) -> usize {
    let stats_width = 3 * 14; // 3 columns × ~14 chars each
    (area_width as usize).saturating_sub(LABEL_W + PCT_W + 2 + stats_width).max(6)
}

/// Render a stat column: `label value` with fixed width.
fn stat(label: &str, value: &str, val_color: Color) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("{:<5}", label), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:>8} ", value), Style::default().fg(val_color)),
    ]
}

/// Render CPU line with aligned columns.
pub fn render_cpu_section(frame: &mut Frame, area: Rect, app: &App) {
    let pct = app.system_metrics.cpu_global as f64;
    let bw = bar_width(area.width);
    let bd = &app.system_metrics.cpu_breakdown;

    let mut s = vec![
        Span::styled("CPU  ", Style::default().fg(Color::Rgb(130, 170, 255)).add_modifier(Modifier::BOLD)),
    ];
    s.extend(gradient_bar(pct, bw));
    s.push(Span::styled(format!(" {:5.1}%", pct), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
    s.push(Span::raw("  "));
    s.extend(stat("usr", &format!("{:.1}%", bd.user), Color::Rgb(130, 170, 255)));
    s.extend(stat("sys", &format!("{:.1}%", bd.system), Color::Rgb(255, 180, 50)));
    s.extend(stat("idl", &format!("{:.1}%", bd.idle), Color::Rgb(80, 220, 120)));

    frame.render_widget(Paragraph::new(Line::from(s)), area);
}

/// Render MEM line with aligned columns.
pub fn render_memory_section(frame: &mut Frame, area: Rect, app: &App) {
    let mem = &app.system_metrics.memory;
    let pct = if mem.total > 0 { (mem.used as f64 / mem.total as f64) * 100.0 } else { 0.0 };
    let bw = bar_width(area.width);

    let mut s = vec![
        Span::styled("MEM  ", Style::default().fg(Color::Rgb(200, 130, 255)).add_modifier(Modifier::BOLD)),
    ];
    s.extend(gradient_bar(pct, bw));
    s.push(Span::styled(format!(" {:5.1}%", pct), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
    s.push(Span::raw("  "));
    s.extend(stat("total", &format_size(mem.total, BINARY), Color::White));
    s.extend(stat("used", &format_size(mem.used, BINARY), Color::Rgb(200, 130, 255)));
    s.extend(stat("free", &format_size(mem.free, BINARY), Color::DarkGray));

    frame.render_widget(Paragraph::new(Line::from(s)), area);
}

/// Render SWAP line with aligned columns.
pub fn render_swap_section(frame: &mut Frame, area: Rect, app: &App) {
    let mem = &app.system_metrics.memory;
    let pct = if mem.swap_total > 0 { (mem.swap_used as f64 / mem.swap_total as f64) * 100.0 } else { 0.0 };
    let bw = bar_width(area.width);

    let mut s = vec![
        Span::styled("SWAP ", Style::default().fg(Color::Rgb(255, 180, 50)).add_modifier(Modifier::BOLD)),
    ];
    s.extend(gradient_bar(pct, bw));
    s.push(Span::styled(format!(" {:5.1}%", pct), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
    s.push(Span::raw("  "));
    s.extend(stat("total", &format_size(mem.swap_total, BINARY), Color::White));
    s.extend(stat("used", &format_size(mem.swap_used, BINARY), Color::Rgb(255, 180, 50)));
    s.extend(stat("free", &format_size(mem.swap_free, BINARY), Color::DarkGray));

    frame.render_widget(Paragraph::new(Line::from(s)), area);
}

/// Render LOAD line with aligned columns.
pub fn render_load_section(frame: &mut Frame, area: Rect, app: &App) {
    let cores = app.system_metrics.cpu_count;
    let load = &app.system_metrics.load_avg;
    let load_pct = if cores > 0 { (load.0 / cores as f64 * 100.0).min(100.0) } else { 0.0 };
    let bw = bar_width(area.width);

    let mut s = vec![
        Span::styled("LOAD ", Style::default().fg(Color::Rgb(255, 220, 100)).add_modifier(Modifier::BOLD)),
    ];
    s.extend(gradient_bar(load_pct, bw));
    s.push(Span::styled(format!(" {:5.1}%", load_pct), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
    s.push(Span::raw("  "));
    s.extend(stat(&format!("{}c", cores), &format!("{:.2}", load.0), load_color(load.0, cores)));
    s.extend(stat("5m", &format!("{:.2}", load.1), load_color(load.1, cores)));
    s.extend(stat("15m", &format!("{:.2}", load.2), load_color(load.2, cores)));

    frame.render_widget(Paragraph::new(Line::from(s)), area);
}

fn load_color(load: f64, cores: usize) -> Color {
    let ratio = load / cores.max(1) as f64;
    if ratio >= 1.0 {
        Color::Rgb(255, 80, 80)
    } else if ratio >= 0.7 {
        Color::Rgb(255, 180, 50)
    } else {
        Color::Rgb(80, 220, 120)
    }
}

/// Render per-core CPU bars in two columns.
pub fn render_per_core_cpu(frame: &mut Frame, area: Rect, app: &App) {
    let cpus = &app.system_metrics.cpus;
    if cpus.is_empty() {
        return;
    }

    let half = (cpus.len() + 1) / 2;
    let col_width = (area.width as usize).saturating_sub(4) / 2;
    let bar_w = col_width.saturating_sub(12);

    let mut lines: Vec<Line> = Vec::new();
    for row_idx in 0..half {
        let mut spans = Vec::new();

        // Left column
        if let Some(cpu) = cpus.get(row_idx) {
            let pct = cpu.usage as f64;
            let filled = ((pct / 100.0) * bar_w as f64).round() as usize;
            let empty = bar_w.saturating_sub(filled);
            let color = gradient_color(pct);
            spans.push(Span::styled(format!("{:>2} ", row_idx), Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled("▓".repeat(filled), Style::default().fg(color)));
            spans.push(Span::styled("░".repeat(empty), Style::default().fg(Color::Rgb(40, 40, 50))));
            spans.push(Span::styled(format!(" {:5.1}%", pct), Style::default().fg(Color::White)));
        }

        // Right column
        let right_idx = row_idx + half;
        if let Some(cpu) = cpus.get(right_idx) {
            let pct = cpu.usage as f64;
            let filled = ((pct / 100.0) * bar_w as f64).round() as usize;
            let empty = bar_w.saturating_sub(filled);
            let color = gradient_color(pct);
            spans.push(Span::raw("  "));
            spans.push(Span::styled(format!("{:>2} ", right_idx), Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled("▓".repeat(filled), Style::default().fg(color)));
            spans.push(Span::styled("░".repeat(empty), Style::default().fg(Color::Rgb(40, 40, 50))));
            spans.push(Span::styled(format!(" {:5.1}%", pct), Style::default().fg(Color::White)));
        }

        lines.push(Line::from(spans));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" Per-Core CPU ({} cores) ", cpus.len()))
        .border_style(Style::default().fg(Color::Rgb(80, 80, 120)));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn gradient_color(pct: f64) -> Color {
    if pct >= 90.0 {
        Color::Rgb(255, 80, 80)
    } else if pct >= 70.0 {
        Color::Rgb(255, 180, 50)
    } else if pct >= 50.0 {
        Color::Rgb(100, 200, 255)
    } else {
        Color::Rgb(80, 220, 120)
    }
}

/// Full-screen network view with all interfaces and detailed stats.
pub fn render_network_full(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["Interface", "▼ Rx/s", "▲ Tx/s", "Total Rx", "Total Tx"])
        .style(Style::default().fg(Color::Rgb(200, 200, 255)).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app.system_metrics.networks.iter().map(|net| {
        Row::new(vec![
            Cell::from(net.interface.clone()).style(Style::default().fg(Color::Cyan)),
            Cell::from(format!("{}/s", format_size(net.rx_rate as u64, BINARY)))
                .style(Style::default().fg(Color::Green)),
            Cell::from(format!("{}/s", format_size(net.tx_rate as u64, BINARY)))
                .style(Style::default().fg(Color::Rgb(255, 100, 100))),
            Cell::from(format_size(net.rx_bytes, BINARY)),
            Cell::from(format_size(net.tx_bytes, BINARY)),
        ])
    }).collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(12),
            Constraint::Min(12),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Network Interfaces ")
            .border_style(Style::default().fg(Color::Rgb(80, 80, 120))),
    )
    .header(header);

    frame.render_widget(table, area);
}

/// Full-screen disk view with all filesystems (df -h style).
pub fn render_disk_full(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["Filesystem", "Size", "Used", "Avail", "Use%", "Mounted on"])
        .style(Style::default().fg(Color::Rgb(200, 200, 255)).add_modifier(Modifier::BOLD));

    let mut rows: Vec<Row> = Vec::new();
    for disk in app.system_metrics.disks.iter().filter(|d| d.total > 0) {
        let pct = (disk.used as f64 / disk.total as f64) * 100.0;
        let avail = disk.total.saturating_sub(disk.used);

        // Color based on usage
        let pct_color = if pct >= 90.0 {
            Color::Rgb(255, 80, 80)
        } else if pct >= 75.0 {
            Color::Rgb(255, 180, 50)
        } else {
            Color::Rgb(80, 220, 120)
        };

        let first_mp = disk.mount_points.first().cloned().unwrap_or_default();
        // Main row with filesystem stats and first mount point
        rows.push(Row::new(vec![
            Cell::from(disk.name.clone()).style(Style::default().fg(Color::Cyan)),
            Cell::from(format_size(disk.total, BINARY)),
            Cell::from(format_size(disk.used, BINARY)),
            Cell::from(format_size(avail, BINARY)),
            Cell::from(format!("{:>3.0}%", pct)).style(Style::default().fg(pct_color)),
            Cell::from(first_mp).style(Style::default().fg(Color::White)),
        ]));
        // Tree rows for additional mount points
        let extra = &disk.mount_points[1..];
        for (i, mp) in extra.iter().enumerate() {
            let prefix = if i == extra.len() - 1 { "└─ " } else { "├─ " };
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(format!("{}{}", prefix, mp)).style(Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    let table = Table::new(
        rows,
        [
            Constraint::Min(25),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Min(15),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Filesystems (df -h) ")
            .border_style(Style::default().fg(Color::Rgb(80, 80, 120))),
    )
    .header(header);

    frame.render_widget(table, area);
}

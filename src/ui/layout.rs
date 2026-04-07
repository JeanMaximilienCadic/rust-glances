//! Main layout and UI coordination.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use super::alerts::render_alerts_panel;
use super::dialogs::{render_help, render_kill_confirm, render_status};
#[cfg(feature = "docker")]
use super::docker::render_docker_panel;
use super::footer::render_footer;
#[cfg(feature = "gpu")]
use super::gpu::render_gpu_panel;
use super::header::render_header;
#[cfg(feature = "docker")]
use super::http_dialog::render_http_dialog;
#[cfg(feature = "docker")]
use super::logs_dialog::render_logs_dialog;
use super::ports::render_port_processes;
use super::processes::render_cpu_processes;
use super::system::{
    render_cpu_section, render_disk_full, render_disk_io_section, render_load_section,
    render_memory_section, render_network_full, render_per_core_cpu, render_gpus_compact,
};
use super::tabs::render_tabs;
use super::temps::render_temps_panel;
use crate::app::{App, ViewTab};

/// Main UI rendering function.
pub fn render_ui(frame: &mut Frame, app: &mut App) {
    if app.kill_confirm.is_some() {
        render_kill_confirm(frame, frame.area(), app);
        return;
    }

    if app.show_help {
        render_help(frame, frame.area());
        return;
    }

    let has_status = app.status_message.is_some();

    // Main vertical: header | spacer | tabs | content | (status) | footer
    let mut constraints = vec![
        Constraint::Length(1), // Header
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Tabs
        Constraint::Min(0),   // Content
    ];
    if has_status {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1)); // Footer

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.area());

    let mut idx = 0;
    render_header(frame, main_chunks[idx], app);
    idx += 1;
    // Skip spacer (idx 1)
    idx += 1;
    render_tabs(frame, main_chunks[idx], app);
    idx += 1;
    let content = main_chunks[idx];
    idx += 1;

    if has_status {
        render_status(frame, main_chunks[idx], app);
        idx += 1;
    }
    render_footer(frame, main_chunks[idx], app);

    // Render active view
    match app.active_tab {
        ViewTab::Overview => render_overview(frame, content, app),
        ViewTab::Processes => render_processes_view(frame, content, app),
        ViewTab::Network => render_network_view(frame, content, app),
        ViewTab::Disks => render_disks_view(frame, content, app),
        ViewTab::Virt => render_virt_view(frame, content, app),
        ViewTab::Gpu => render_gpu_view(frame, content, app),
        ViewTab::Ports => render_ports_view(frame, content, app),
    }

    // Render overlays on top
    #[cfg(feature = "docker")]
    {
        let full = frame.area();
        render_http_dialog(frame, full, app);
        render_logs_dialog(frame, full, app);
    }
}

/// Overview: the main dashboard — CPU/MEM/DISK/LOAD left, Network/Temps right, GPUs, processes, alerts bottom.
fn render_overview(frame: &mut Frame, area: Rect, app: &mut App) {
    let ongoing_alerts = app.alerts.iter().filter(|a| a.ongoing).count();
    let gpu_count = app.gpu_metrics.as_ref().map(|g| g.gpus.len()).unwrap_or(0);
    let has_gpus = gpu_count > 0;

    // Split: top info panels | GPUs | per-core | processes | alerts (always at bottom)
    let mut constraints = vec![
        Constraint::Length(7),  // Top info row (CPU + MEM + DISK + LOAD | Network)
    ];
    if has_gpus {
        // Each GPU gets 1 line + 2 for border
        let gpu_height = (gpu_count as u16 + 2).min(6);
        constraints.push(Constraint::Length(gpu_height));
    }
    if app.show_per_core {
        let core_rows = (app.system_metrics.cpus.len().div_ceil(2) + 2) as u16;
        constraints.push(Constraint::Length(core_rows.min(10)));
    }
    // Alerts always at bottom - height based on ongoing alerts or minimum of 3
    let alerts_height = if ongoing_alerts > 0 {
        (ongoing_alerts as u16 + 2).min(6)
    } else {
        3 // Minimum height to show "No active alerts"
    };
    constraints.push(Constraint::Min(5)); // Processes (takes remaining space, min 5 lines)
    constraints.push(Constraint::Length(alerts_height)); // Alerts fixed at bottom

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut ci = 0;

    // Top row: left (CPU/MEM/DISK/LOAD) | right (Network/Sensors)
    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(v_chunks[ci]);
    ci += 1;

    // Left column: CPU, MEM, DISK I/O, LOAD stacked
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // CPU
            Constraint::Length(1), // MEM
            Constraint::Length(1), // DISK I/O
            Constraint::Length(1), // LOAD
            Constraint::Min(0),   // spacer
        ])
        .margin(1)
        .split(top_cols[0]);

    render_cpu_section(frame, left_rows[0], app);
    render_memory_section(frame, left_rows[1], app);
    render_disk_io_section(frame, left_rows[2], app);
    render_load_section(frame, left_rows[3], app);

    // Right column: Network interfaces + temps side by side
    let has_temps = app.show_temps && !app.system_metrics.temperatures.is_empty();
    if has_temps {
        let right_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(top_cols[1]);
        render_network_compact(frame, right_cols[0], app);
        render_temps_panel(frame, right_cols[1], app);
    } else {
        render_network_compact(frame, top_cols[1], app);
    }

    // GPUs (all of them, separately)
    if has_gpus {
        render_gpus_compact(frame, v_chunks[ci], app);
        ci += 1;
    }

    // Per-core
    if app.show_per_core {
        render_per_core_cpu(frame, v_chunks[ci], app);
        ci += 1;
    }

    // Processes
    render_cpu_processes(frame, v_chunks[ci], app);
    ci += 1;

    // Alerts (always at bottom)
    render_alerts_panel(frame, v_chunks[ci], app);

    let _ = ci;
}

/// Full-screen process view.
fn render_processes_view(frame: &mut Frame, area: Rect, app: &mut App) {
    render_cpu_processes(frame, area, app);
}

/// Full-screen network view with all interfaces, port forwards, and graph.
fn render_network_view(frame: &mut Frame, area: Rect, app: &mut App) {
    let has_port_forwards = !app.system_metrics.port_forwards.is_empty();

    let constraints = if has_port_forwards {
        vec![
            Constraint::Length(8),  // Network graph
            Constraint::Min(5),     // Network interfaces
            Constraint::Length(6),  // Port forwards
        ]
    } else {
        vec![
            Constraint::Length(8),  // Network graph
            Constraint::Min(5),     // Network interfaces
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    render_network_graph(frame, chunks[0], app);
    render_network_full(frame, chunks[1], app);

    if has_port_forwards {
        render_port_forwards(frame, chunks[2], app);
    }
}

/// Render port forwarding rules panel.
fn render_port_forwards(frame: &mut Frame, area: Rect, app: &App) {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::widgets::{Block, Borders, Cell, Row, Table, block::BorderType};

    let header = Row::new(vec!["Proto", "External", "→", "Internal", "Source"])
        .style(Style::default().fg(Color::Rgb(200, 200, 255)).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app.system_metrics.port_forwards.iter().map(|rule| {
        let proto_color = if rule.protocol == "tcp" {
            Color::Cyan
        } else {
            Color::Magenta
        };

        Row::new(vec![
            Cell::from(rule.protocol.clone()).style(Style::default().fg(proto_color)),
            Cell::from(format!(":{}", rule.src_port)).style(Style::default().fg(Color::Green)),
            Cell::from("→").style(Style::default().fg(Color::DarkGray)),
            Cell::from(format!("{}:{}", rule.dest_ip, rule.dest_port))
                .style(Style::default().fg(Color::Rgb(255, 180, 50))),
            Cell::from(rule.src_ip.clone()).style(Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),   // Proto
            Constraint::Length(8),   // External port
            Constraint::Length(2),   // Arrow
            Constraint::Length(22),  // Internal IP:port
            Constraint::Min(10),     // Source
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(" Port Forwards ({}) ", app.system_metrics.port_forwards.len()))
            .border_style(Style::default().fg(Color::Rgb(80, 120, 80))),
    )
    .header(header);

    frame.render_widget(table, area);
}

/// Full-screen disk view.
fn render_disks_view(frame: &mut Frame, area: Rect, app: &mut App) {
    render_disk_full(frame, area, app);
}

/// Full-screen virtualization view (Docker, LXC, Lima).
#[allow(unused_variables)]
fn render_virt_view(frame: &mut Frame, area: Rect, app: &mut App) {
    #[cfg(feature = "docker")]
    render_virt_panel(frame, area, app);
    #[cfg(not(feature = "docker"))]
    render_feature_disabled(frame, area, "Virtualization", "docker");
}

/// Unified virtualization panel showing Docker/LXC/Lima containers.
#[cfg(feature = "docker")]
fn render_virt_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    // Layout imports reserved for future multi-runtime support

    // For now, show Docker panel (LXC and Lima can be added later)
    // Split area if we have multiple container types
    let has_docker = !app.docker_containers.is_empty();

    if has_docker {
        // Show Docker containers
        render_docker_panel(frame, area, app);
    } else {
        // Show "no containers" message
        use ratatui::style::{Color, Style};
        use ratatui::text::Line;
        use ratatui::widgets::{Block, Borders, Paragraph, block::BorderType};

        let text = vec![
            Line::from(""),
            Line::from("No containers detected."),
            Line::from(""),
            Line::from("Supported runtimes:"),
            Line::from("  • Docker (enabled)"),
            Line::from("  • LXC (planned)"),
            Line::from("  • Lima (planned)"),
        ];
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Virtualization ")
            .border_style(Style::default().fg(Color::DarkGray));
        frame.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(Color::DarkGray))
                .block(block),
            area,
        );
    }
}

/// Full-screen GPU view.
#[allow(unused_variables)]
fn render_gpu_view(frame: &mut Frame, area: Rect, app: &mut App) {
    #[cfg(feature = "gpu")]
    render_gpu_panel(frame, area, app);
    #[cfg(not(feature = "gpu"))]
    render_feature_disabled(frame, area, "GPU", "gpu");
}

/// Render a "feature disabled" placeholder.
#[allow(dead_code)]
fn render_feature_disabled(frame: &mut Frame, area: Rect, name: &str, feature: &str) {
    use ratatui::style::{Color, Style};
    use ratatui::text::Line;
    use ratatui::widgets::{Block, Borders, Paragraph, block::BorderType};

    let text = vec![
        Line::from(""),
        Line::from(format!("{} monitoring is disabled.", name)),
        Line::from(""),
        Line::from(format!("Rebuild with: cargo install glances --features {}", feature)),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {} (disabled) ", name))
        .border_style(Style::default().fg(Color::DarkGray));
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray))
            .block(block),
        area,
    );
}

/// Compact network list for overview sidebar.
fn render_network_compact(frame: &mut Frame, area: Rect, app: &App) {
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Paragraph, block::BorderType};

    let mut lines: Vec<Line> = Vec::new();
    let max_ifaces = (area.height as usize).saturating_sub(2);

    for net in app.system_metrics.networks.iter().take(max_ifaces) {
        let rx = format_rate(net.rx_rate);
        let tx = format_rate(net.tx_rate);
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<10}", truncate(&net.interface, 10)),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!(" {:>8}", rx),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!(" {:>8}", tx),
                Style::default().fg(Color::Rgb(255, 100, 100)),
            ),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Network ▼Rx  ▲Tx")
        .border_style(Style::default().fg(Color::Rgb(80, 80, 120)));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Network throughput sparkline graph.
fn render_network_graph(frame: &mut Frame, area: Rect, app: &App) {
    use ratatui::style::{Color, Style};
    use ratatui::symbols;
    use ratatui::text::Line;
    use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, block::BorderType};

    let rx_data: Vec<(f64, f64)> = app.history.network_rx_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v)).collect();
    let tx_data: Vec<(f64, f64)> = app.history.network_tx_history.iter().enumerate()
        .map(|(i, &v)| (i as f64, v)).collect();

    let max_val = rx_data.iter().chain(tx_data.iter())
        .map(|(_, v)| *v)
        .fold(0.1_f64, f64::max);

    let datasets = vec![
        Dataset::default()
            .name("▼ Rx")
            .marker(symbols::Marker::Braille)
            .graph_type(ratatui::widgets::GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(&rx_data),
        Dataset::default()
            .name("▲ Tx")
            .marker(symbols::Marker::Braille)
            .graph_type(ratatui::widgets::GraphType::Line)
            .style(Style::default().fg(Color::Rgb(255, 100, 100)))
            .data(&tx_data),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Network (MB/s)")
                .border_style(Style::default().fg(Color::Rgb(80, 80, 120))),
        )
        .x_axis(Axis::default().bounds([0.0, 59.0]).labels::<Vec<Line>>(vec![]))
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::DarkGray))
                .bounds([0.0, max_val * 1.1])
                .labels(vec![
                    Line::from("0"),
                    Line::from(format!("{:.1}", max_val / 2.0)),
                    Line::from(format!("{:.1}", max_val)),
                ]),
        );

    frame.render_widget(chart, area);
}

/// Format network rate to human-readable string.
fn format_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_073_741_824.0 {
        format!("{:.1}GB", bytes_per_sec / 1_073_741_824.0)
    } else if bytes_per_sec >= 1_048_576.0 {
        format!("{:.1}MB", bytes_per_sec / 1_048_576.0)
    } else if bytes_per_sec >= 1024.0 {
        format!("{:.0}KB", bytes_per_sec / 1024.0)
    } else {
        format!("{:.0}b", bytes_per_sec)
    }
}

/// Full-screen ports view.
fn render_ports_view(frame: &mut Frame, area: Rect, app: &mut App) {
    render_port_processes(frame, area, app);
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { s[..max].to_string() }
}

/// Create a centered rectangle for dialogs.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

//! Docker container panel — grouped by compose project, with details panel for ports/volumes.

use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table, block::BorderType,
    },
    Frame,
};

use crate::app::App;

/// Format size compactly with fixed width (6 chars) for table alignment.
fn compact_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;
    const TIB: u64 = 1024 * 1024 * 1024 * 1024;

    if bytes >= TIB {
        format!("{:>5}T", bytes / TIB)
    } else if bytes >= GIB {
        format!("{:>5}G", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:>5}M", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:>5}K", bytes / KIB)
    } else {
        format!("{:>5}B", bytes)
    }
}

fn docker_color(pct: f64) -> Color {
    if pct >= 80.0 {
        Color::Rgb(255, 80, 80)
    } else if pct >= 50.0 {
        Color::Rgb(255, 180, 50)
    } else {
        Color::Rgb(80, 220, 120)
    }
}

/// Truncate string to max length with ellipsis.
fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max <= 3 {
        s.chars().take(max).collect()
    } else {
        format!("{}...", s.chars().take(max - 3).collect::<String>())
    }
}

/// Render the Docker containers table with compose grouping and selection.
pub fn render_docker_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.docker_containers.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Docker (no running containers) ")
            .border_style(Style::default().fg(Color::DarkGray));
        frame.render_widget(block, area);
        return;
    }

    // Split: container table (top) | details panel (bottom)
    let has_selection = app.docker_state.selected().is_some();
    let details_height = if has_selection { 8u16 } else { 0u16 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),
            Constraint::Length(details_height),
        ])
        .split(area);

    render_container_table(frame, chunks[0], app);

    if has_selection && details_height > 0 {
        render_details_panel(frame, chunks[1], app);
    }
}

/// Render the container table.
fn render_container_table(frame: &mut Frame, area: Rect, app: &mut App) {
    // Determine compose grouping
    let has_compose = app.docker_containers.iter().any(|c| !c.compose_project.is_empty());

    let header_cells = if has_compose {
        vec!["Compose", "Service", "Status", "CPU%", "MEM", "Net I/O", "Blk I/O", "Ports", "Vols"]
    } else {
        vec!["Name", "Image", "Status", "CPU%", "MEM", "Net I/O", "Blk I/O", "Ports", "Vols"]
    };

    let header = Row::new(header_cells)
        .style(
            Style::default()
                .fg(Color::Rgb(200, 200, 255))
                .add_modifier(Modifier::BOLD),
        );

    // Sort containers: group by compose project, then by name
    let mut sorted_indices: Vec<usize> = (0..app.docker_containers.len()).collect();
    sorted_indices.sort_by(|&a, &b| {
        let ca = &app.docker_containers[a];
        let cb = &app.docker_containers[b];
        ca.compose_project
            .cmp(&cb.compose_project)
            .then(ca.name.cmp(&cb.name))
    });

    let mut rows: Vec<Row> = Vec::new();
    let mut last_project = String::new();

    for &idx in &sorted_indices {
        let c = &app.docker_containers[idx];

        let state_color = if c.state == "running" {
            Color::Rgb(80, 220, 120)
        } else {
            Color::Rgb(255, 180, 50)
        };

        let mem_str = format_size(c.memory_used, BINARY);

        // Calculate dynamic truncation based on area width
        let col1_max = (area.width as usize * 14 / 100).max(12);
        let col2_max = (area.width as usize * 16 / 100).max(14);

        let (col1, col2) = if has_compose {
            let project_display = if c.compose_project != last_project && !c.compose_project.is_empty() {
                last_project = c.compose_project.clone();
                truncate_str(&c.compose_project, col1_max)
            } else if c.compose_project.is_empty() {
                truncate_str(&c.name, col1_max)
            } else {
                "  └".into()
            };

            let service = if !c.compose_service.is_empty() {
                truncate_str(&c.compose_service, col2_max)
            } else {
                truncate_str(&c.name, col2_max)
            };

            (project_display, service)
        } else {
            (truncate_str(&c.name, col1_max), truncate_str(&c.image, col2_max))
        };

        // Network I/O combined - fixed 6-char values for alignment
        let net_io = format!("↓{} ↑{}", compact_size(c.net_rx), compact_size(c.net_tx));

        // Block I/O combined - fixed 6-char values for alignment
        let blk_io = format!("R{} W{}", compact_size(c.block_read), compact_size(c.block_write));

        // Port and volume counts
        let port_count = c.port_mappings.len();
        let vol_count = c.volume_mounts.len();

        rows.push(Row::new(vec![
            Cell::from(col1).style(Style::default().fg(Color::Cyan)),
            Cell::from(col2).style(Style::default().fg(Color::Rgb(130, 170, 255))),
            Cell::from(c.state.clone()).style(Style::default().fg(state_color)),
            Cell::from(format!("{:.1}%", c.cpu_percent))
                .style(Style::default().fg(docker_color(c.cpu_percent))),
            Cell::from(mem_str),
            Cell::from(net_io).style(Style::default().fg(Color::DarkGray)),
            Cell::from(blk_io).style(Style::default().fg(Color::DarkGray)),
            Cell::from(format!("{}", port_count))
                .style(Style::default().fg(if port_count > 0 { Color::Green } else { Color::DarkGray })),
            Cell::from(format!("{}", vol_count))
                .style(Style::default().fg(if vol_count > 0 { Color::Magenta } else { Color::DarkGray })),
        ]));
    }

    // Title with container count
    let title = Line::from(vec![
        Span::styled(
            format!(" Docker — {} containers ", app.docker_containers.len()),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            "↑↓:select  Enter:HTTP  l:logs",
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    // Use flexible constraints to fill available width
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(13),  // Compose/Name
            Constraint::Percentage(15),  // Service/Image
            Constraint::Percentage(9),   // Status
            Constraint::Percentage(7),   // CPU%
            Constraint::Percentage(9),   // MEM
            Constraint::Percentage(16),  // Net I/O (fixed-width aligned)
            Constraint::Percentage(16),  // Blk I/O (fixed-width aligned)
            Constraint::Percentage(7),   // Ports count
            Constraint::Percentage(8),   // Vols count
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title)
            .border_style(Style::default().fg(Color::Rgb(80, 120, 180))),
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 40, 60))
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, area, &mut app.docker_state);

    // Scrollbar
    if app.docker_containers.len() > (area.height as usize).saturating_sub(3) {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(app.docker_containers.len())
            .position(app.docker_state.selected().unwrap_or(0));

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

/// Render the details panel showing ports and volumes for selected container.
fn render_details_panel(frame: &mut Frame, area: Rect, app: &App) {
    let Some(container) = app.get_selected_container() else {
        return;
    };

    // Adjust split based on content - give more space to volumes if there are many
    let port_count = container.port_mappings.len();
    let vol_count = container.volume_mounts.len();

    let (port_pct, vol_pct) = if vol_count > port_count * 2 {
        (30, 70) // More volumes - give them more space
    } else if port_count > vol_count * 2 {
        (50, 50) // More ports - balance it out
    } else {
        (35, 65) // Default - volumes usually have longer paths
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(port_pct), Constraint::Percentage(vol_pct)])
        .split(area);

    // Ports panel
    render_ports_panel(frame, chunks[0], container);

    // Volumes panel
    render_volumes_panel(frame, chunks[1], container);
}

/// Render ports for a container.
fn render_ports_panel(frame: &mut Frame, area: Rect, container: &crate::metrics::docker::ContainerInfo) {
    let mut lines: Vec<Line> = Vec::new();

    if container.port_mappings.is_empty() {
        lines.push(Line::from(Span::styled(
            "No port mappings",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let max_lines = (area.height as usize).saturating_sub(2);
        let total_ports = container.port_mappings.len();

        for (i, port) in container.port_mappings.iter().take(max_lines) .enumerate() {
            let host = if port.host_ip.is_empty() || port.host_ip == "0.0.0.0" {
                format!("{}", port.host_port)
            } else {
                format!("{}:{}", port.host_ip, port.host_port)
            };

            lines.push(Line::from(vec![
                Span::styled(host, Style::default().fg(Color::Green)),
                Span::styled(" → ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}/{}", port.container_port, port.protocol),
                    Style::default().fg(Color::Cyan),
                ),
            ]));

            // Show "+N more" if truncated
            if i == max_lines - 1 && total_ports > max_lines {
                lines.push(Line::from(Span::styled(
                    format!("+{} more...", total_ports - max_lines),
                    Style::default().fg(Color::DarkGray),
                )));
                break;
            }
        }
    }

    let title = format!(" Ports ({}) ", container.port_mappings.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(Style::default().fg(Color::Rgb(80, 120, 80)));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Render volumes for a container.
fn render_volumes_panel(frame: &mut Frame, area: Rect, container: &crate::metrics::docker::ContainerInfo) {
    let mut lines: Vec<Line> = Vec::new();

    if container.volume_mounts.is_empty() {
        lines.push(Line::from(Span::styled(
            "No volume mounts",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let max_lines = (area.height as usize).saturating_sub(2);
        let total_vols = container.volume_mounts.len();
        let inner_width = area.width.saturating_sub(4) as usize;

        for (i, vol) in container.volume_mounts.iter().take(max_lines).enumerate() {
            let mode_color = if vol.mode == "ro" {
                Color::Yellow
            } else {
                Color::Green
            };

            let type_indicator = match vol.mount_type.as_str() {
                "volume" => "V",
                "tmpfs" => "T",
                _ => "B",
            };

            // Calculate available space for paths
            // Format: [T] source → dest (rw)
            let fixed_chars = 6 + 4 + 5; // "[T] " + " → " + " (rw)"
            let available = inner_width.saturating_sub(fixed_chars);

            // Split available space: 55% for source, 45% for dest
            let src_max = (available * 55 / 100).max(8);
            let dst_max = (available * 45 / 100).max(8);

            let src_display = truncate_str(&vol.source, src_max);
            let dst_display = truncate_str(&vol.destination, dst_max);

            lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}] ", type_indicator),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(src_display, Style::default().fg(Color::Magenta)),
                Span::styled(" → ", Style::default().fg(Color::DarkGray)),
                Span::styled(dst_display, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!(" ({})", vol.mode),
                    Style::default().fg(mode_color),
                ),
            ]));

            // Show "+N more" if truncated
            if i == max_lines - 1 && total_vols > max_lines {
                lines.push(Line::from(Span::styled(
                    format!("+{} more...", total_vols - max_lines),
                    Style::default().fg(Color::DarkGray),
                )));
                break;
            }
        }
    }

    let title = format!(" Volumes ({}) — B:bind V:vol T:tmpfs ", container.volume_mounts.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(Style::default().fg(Color::Rgb(120, 80, 120)));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

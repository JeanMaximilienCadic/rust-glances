//! Docker container panel — grouped by compose project, selectable with arrows.

use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
        block::BorderType,
    },
    Frame,
};

use crate::app::App;
use crate::utils::format_duration;

fn docker_color(pct: f64) -> Color {
    if pct >= 80.0 {
        Color::Rgb(255, 80, 80)
    } else if pct >= 50.0 {
        Color::Rgb(255, 180, 50)
    } else {
        Color::Rgb(80, 220, 120)
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

    // Determine compose grouping
    let has_compose = app.docker_containers.iter().any(|c| !c.compose_project.is_empty());

    let header_cells = if has_compose {
        vec!["Compose", "Service", "Status", "Uptime", "CPU%", "MEM/MAX", "IOR/s", "IOW/s", "▼Rx/s", "▲Tx/s", "Ports"]
    } else {
        vec!["Name", "Image", "Status", "Uptime", "CPU%", "MEM/MAX", "IOR/s", "IOW/s", "▼Rx/s", "▲Tx/s", "Ports"]
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

        let uptime = format_duration(c.uptime_secs);

        let mem_max = if c.memory_limit > 0 {
            format!(
                "{}/{}",
                format_size(c.memory_used, BINARY),
                format_size(c.memory_limit, BINARY)
            )
        } else {
            format_size(c.memory_used, BINARY)
        };

        let (col1, col2) = if has_compose {
            // Show compose project (only on first row of group)
            let project_display = if c.compose_project != last_project && !c.compose_project.is_empty() {
                last_project = c.compose_project.clone();
                c.compose_project.clone()
            } else if c.compose_project.is_empty() {
                c.name.clone()
            } else {
                "  └".into()
            };

            let service = if !c.compose_service.is_empty() {
                c.compose_service.clone()
            } else {
                c.name.clone()
            };

            (project_display, service)
        } else {
            (c.name.clone(), c.image.clone())
        };

        rows.push(Row::new(vec![
            Cell::from(col1).style(Style::default().fg(Color::Cyan)),
            Cell::from(col2).style(Style::default().fg(Color::Rgb(130, 170, 255))),
            Cell::from(c.state.clone()).style(Style::default().fg(state_color)),
            Cell::from(uptime),
            Cell::from(format!("{:.1}", c.cpu_percent))
                .style(Style::default().fg(docker_color(c.cpu_percent))),
            Cell::from(mem_max),
            Cell::from(format!("{}/s", format_size(c.block_read, BINARY))),
            Cell::from(format!("{}/s", format_size(c.block_write, BINARY))),
            Cell::from(format!("{}/s", format_size(c.net_rx, BINARY))),
            Cell::from(format!("{}/s", format_size(c.net_tx, BINARY))),
            Cell::from(c.ports.clone()).style(Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Title with compose dir if all from same project
    let compose_info = if has_compose {
        let projects: Vec<&str> = app.docker_containers.iter()
            .filter(|c| !c.compose_project.is_empty())
            .map(|c| c.compose_dir.as_str())
            .collect();
        if !projects.is_empty() && projects.iter().all(|p| *p == projects[0]) && !projects[0].is_empty() {
            format!(" — compose: {}", projects[0])
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let title = Line::from(vec![
        Span::styled(
            format!(" Docker — {} containers ", app.docker_containers.len()),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            compose_info,
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let table = Table::new(
        rows,
        [
            Constraint::Length(18),  // Compose/Name
            Constraint::Length(20),  // Service/Image
            Constraint::Length(9),   // Status
            Constraint::Length(12),  // Uptime
            Constraint::Length(6),   // CPU%
            Constraint::Length(18),  // MEM/MAX
            Constraint::Length(10),  // IOR/s
            Constraint::Length(10),  // IOW/s
            Constraint::Length(10),  // Rx/s
            Constraint::Length(10),  // Tx/s
            Constraint::Min(0),     // Ports — all remaining width
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

//! Docker container panel rendering — modern style with ports and uptime.

use humansize::{format_size, BINARY};
use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, block::BorderType},
    Frame,
};
use ratatui::layout::Rect;

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

/// Render the Docker containers table.
pub fn render_docker_panel(frame: &mut Frame, area: Rect, app: &App) {
    if app.docker_containers.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Docker (no running containers) ")
            .border_style(Style::default().fg(Color::DarkGray));
        frame.render_widget(block, area);
        return;
    }

    let header = Row::new(vec![
        "Name", "Status", "Uptime", "CPU%", "MEM/MAX", "IOR/s", "IOW/s", "▼Rx/s", "▲Tx/s", "Ports",
    ])
    .style(
        Style::default()
            .fg(Color::Rgb(200, 200, 255))
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .docker_containers
        .iter()
        .map(|c| {
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
                format!("{}", format_size(c.memory_used, BINARY))
            };

            Row::new(vec![
                Cell::from(c.name.clone()).style(Style::default().fg(Color::Cyan)),
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
            ])
        })
        .collect();

    let title = format!(
        " Docker — {} containers (served by docker) ",
        app.docker_containers.len()
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Length(9),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(16),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Min(18),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title)
            .border_style(Style::default().fg(Color::Rgb(80, 120, 180))),
    )
    .header(header);

    frame.render_widget(table, area);
}

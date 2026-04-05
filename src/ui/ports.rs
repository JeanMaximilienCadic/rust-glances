//! Port processes table rendering.

use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table, block::BorderType,
    },
    Frame,
};

use crate::app::App;

/// Render the port processes table.
pub fn render_port_processes(frame: &mut Frame, area: Rect, app: &mut App) {
    app.ports_area = Some(area);

    let procs = &app.port_processes;

    let header = Row::new(vec![
        "PORT",
        "PROTO",
        "PID",
        "USER",
        "CPU%",
        "MEM",
        "BIND",
        "NAME",
        "COMMAND",
    ])
    .style(
        Style::default()
            .fg(Color::Rgb(200, 200, 255))
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = procs
        .iter()
        .map(|p| {
            let proto_color = if p.protocol == "tcp6" {
                Color::Rgb(180, 140, 255)
            } else {
                Color::Rgb(80, 220, 120)
            };

            Row::new(vec![
                Cell::from(format!("{}", p.port))
                    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from(p.protocol.clone()).style(Style::default().fg(proto_color)),
                Cell::from(format!("{}", p.pid)),
                Cell::from(p.user.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{:.1}", p.cpu_usage))
                    .style(Style::default().fg(crate::utils::usage_color(p.cpu_usage as f64))),
                Cell::from(format_size(p.memory_bytes, BINARY)),
                Cell::from(p.bind_address.clone())
                    .style(Style::default().fg(Color::DarkGray)),
                Cell::from(p.name.clone())
                    .style(Style::default().fg(Color::Rgb(80, 220, 120))),
                Cell::from(p.command.clone())
                    .style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let title = format!(
        " Listening Ports ({}) — Ctrl+K kill, Ctrl+T term ",
        procs.len(),
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),  // PORT
            Constraint::Length(6),  // PROTO
            Constraint::Length(8),  // PID
            Constraint::Length(8),  // USER
            Constraint::Length(6),  // CPU%
            Constraint::Length(9),  // MEM
            Constraint::Length(16), // BIND
            Constraint::Length(15), // NAME
            Constraint::Min(0),    // COMMAND
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title)
            .border_style(Style::default().fg(Color::Rgb(130, 170, 255))),
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 40, 60))
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, area, &mut app.ports_state);

    // Scrollbar
    if procs.len() > (area.height as usize).saturating_sub(3) {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("\u{2191}"))
            .end_symbol(Some("\u{2193}"));

        let mut scrollbar_state = ScrollbarState::new(procs.len())
            .position(app.ports_state.selected().unwrap_or(0));

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

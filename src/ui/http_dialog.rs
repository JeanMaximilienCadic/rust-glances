//! HTTP request overlay dialog — Postman-like API testing.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap, block::BorderType},
    Frame,
};

use crate::app::App;

/// Render the HTTP request dialog overlay.
pub fn render_http_dialog(frame: &mut Frame, area: Rect, app: &App) {
    if !app.http_request.visible {
        return;
    }

    let dialog_area = centered_rect(80, 80, area);
    frame.render_widget(Clear, dialog_area);

    let method = app.http_request.method.as_str();
    let method_color = match method {
        "GET" => Color::Rgb(80, 220, 120),
        "POST" => Color::Rgb(255, 180, 50),
        "PUT" => Color::Rgb(100, 200, 255),
        "DELETE" => Color::Rgb(255, 80, 80),
        _ => Color::White,
    };

    let title = format!(
        " API Request → {} (port {}) ",
        app.http_request.container_name, app.http_request.container_port
    );

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(Style::default().fg(Color::Rgb(130, 170, 255)));

    let inner = outer.inner(dialog_area);
    frame.render_widget(outer, dialog_area);

    // Split: method+path | headers | body | response
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Method + Path
            Constraint::Length(4),  // Headers
            Constraint::Length(4),  // Body
            Constraint::Min(3),    // Response
            Constraint::Length(1), // Help
        ])
        .split(inner);

    let af = app.http_request.active_field;
    let editing = app.http_request.editing;

    // Method + Path
    let method_style = if af == 0 {
        Style::default().fg(Color::Black).bg(method_color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(method_color).add_modifier(Modifier::BOLD)
    };
    let path_border = if af == 1 {
        if editing { Color::Rgb(255, 220, 100) } else { Color::Rgb(130, 170, 255) }
    } else {
        Color::Rgb(60, 60, 80)
    };

    let url_line = Line::from(vec![
        Span::styled(format!(" {} ", method), method_style),
        Span::raw(" "),
        Span::styled(
            format!("http://localhost:{}", app.http_request.container_port),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            &app.http_request.path,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        if af == 1 && editing { Span::styled("█", Style::default().fg(Color::Rgb(255, 220, 100))) } else { Span::raw("") },
    ]);
    let url_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Method + Path ")
        .border_style(Style::default().fg(path_border));
    frame.render_widget(Paragraph::new(url_line).block(url_block), chunks[0]);

    // Headers
    let hdr_border = if af == 2 {
        if editing { Color::Rgb(255, 220, 100) } else { Color::Rgb(130, 170, 255) }
    } else {
        Color::Rgb(60, 60, 80)
    };
    let hdr_text = if app.http_request.headers.is_empty() {
        "(no headers)".to_string()
    } else {
        app.http_request.headers.clone()
    };
    let hdr_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Headers ")
        .border_style(Style::default().fg(hdr_border));
    frame.render_widget(
        Paragraph::new(hdr_text).wrap(Wrap { trim: false }).block(hdr_block),
        chunks[1],
    );

    // Body
    let body_border = if af == 3 {
        if editing { Color::Rgb(255, 220, 100) } else { Color::Rgb(130, 170, 255) }
    } else {
        Color::Rgb(60, 60, 80)
    };
    let body_text = if app.http_request.body.is_empty() {
        "(empty body)".to_string()
    } else {
        app.http_request.body.clone()
    };
    let body_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Body (JSON) ")
        .border_style(Style::default().fg(body_border));
    frame.render_widget(
        Paragraph::new(body_text).wrap(Wrap { trim: false }).block(body_block),
        chunks[2],
    );

    // Response
    let resp_title = if let Some(status) = app.http_request.response_status {
        let color = if status < 300 { Color::Rgb(80, 220, 120) }
            else if status < 400 { Color::Rgb(255, 180, 50) }
            else { Color::Rgb(255, 80, 80) };
        Line::from(vec![
            Span::raw(" Response "),
            Span::styled(format!("{}", status), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
        ])
    } else {
        Line::from(" Response ")
    };
    let resp_text = app.http_request.response.as_deref().unwrap_or("(press 's' to send request)");
    let resp_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(resp_title)
        .border_style(Style::default().fg(Color::Rgb(80, 80, 120)));
    frame.render_widget(
        Paragraph::new(resp_text).wrap(Wrap { trim: false }).block(resp_block),
        chunks[3],
    );

    // Help bar
    let help = Line::from(vec![
        Span::styled(" Tab", Style::default().fg(Color::Rgb(130, 170, 255))),
        Span::styled(":Navigate ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Rgb(130, 170, 255))),
        Span::styled(":Edit ", Style::default().fg(Color::DarkGray)),
        Span::styled("m", Style::default().fg(Color::Rgb(130, 170, 255))),
        Span::styled(":Method ", Style::default().fg(Color::DarkGray)),
        Span::styled("s", Style::default().fg(Color::Rgb(80, 220, 120))),
        Span::styled(":Send ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Rgb(255, 80, 80))),
        Span::styled(":Close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[4]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

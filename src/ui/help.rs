//! Pure render function for the help view.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph},
    Frame,
};

/// Render the help overlay.
pub fn render(frame: &mut Frame, area: Rect) {
    // Center the help popup
    let popup_area = centered_rect(60, 70, area);

    // Clear the background
    frame.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Navigation", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  j / ↓    ", Style::default().fg(Color::Yellow)),
            Span::raw("Move down"),
        ]),
        Line::from(vec![
            Span::styled("  k / ↑    ", Style::default().fg(Color::Yellow)),
            Span::raw("Move up"),
        ]),
        Line::from(vec![
            Span::styled("  g        ", Style::default().fg(Color::Yellow)),
            Span::raw("Go to top"),
        ]),
        Line::from(vec![
            Span::styled("  G        ", Style::default().fg(Color::Yellow)),
            Span::raw("Go to bottom"),
        ]),
        Line::from(vec![
            Span::styled("  PgUp     ", Style::default().fg(Color::Yellow)),
            Span::raw("Page up"),
        ]),
        Line::from(vec![
            Span::styled("  PgDn     ", Style::default().fg(Color::Yellow)),
            Span::raw("Page down"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Views", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(Color::Yellow)),
            Span::raw("Select / drill down"),
        ]),
        Line::from(vec![
            Span::styled("  Esc      ", Style::default().fg(Color::Yellow)),
            Span::raw("Go back"),
        ]),
        Line::from(vec![
            Span::styled("  d        ", Style::default().fg(Color::Yellow)),
            Span::raw("Full diff view (PR diff)"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Diff View", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  n        ", Style::default().fg(Color::Yellow)),
            Span::raw("Next file"),
        ]),
        Line::from(vec![
            Span::styled("  p        ", Style::default().fg(Color::Yellow)),
            Span::raw("Previous file"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("General", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  ?        ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("  q        ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .padding(Padding::uniform(1))
                .style(Style::default().bg(Color::Black)),
        )
        .alignment(Alignment::Left);

    frame.render_widget(help, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

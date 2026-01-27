//! Pure render functions for the commits view.

use crate::domain::Commit;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Padding, Row, Table, TableState},
    Frame,
};

/// Render the commits list view.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    commits: &[Commit],
    selected: usize,
    branch: &str,
    base: &str,
    table_state: &mut TableState,
) {
    let title = format!(
        " rev: {} → {} ({} commits) ",
        branch,
        base,
        commits.len()
    );

    let header = Row::new(vec![
        Cell::from("Hash").style(Style::default().fg(Color::DarkGray)),
        Cell::from("Message").style(Style::default().fg(Color::DarkGray)),
        Cell::from("Time").style(Style::default().fg(Color::DarkGray)),
    ])
    .height(1);

    let rows: Vec<Row> = commits
        .iter()
        .enumerate()
        .map(|(i, commit)| {
            let style = if i == selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(Span::styled(
                    &commit.short_hash,
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(truncate(commit.summary(), 60)),
                Cell::from(Span::styled(
                    commit.relative_time(),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .style(style)
        })
        .collect();

    table_state.select(Some(selected));

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(30),
            Constraint::Length(16),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .padding(Padding::horizontal(1)),
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_stateful_widget(table, area, table_state);
}

/// Render the help bar at the bottom.
pub fn render_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(Color::Yellow)),
        Span::raw(" View commit  "),
        Span::styled("[d]", Style::default().fg(Color::Yellow)),
        Span::raw(" Full diff  "),
        Span::styled("[q]", Style::default().fg(Color::Yellow)),
        Span::raw(" Quit  "),
        Span::styled("[?]", Style::default().fg(Color::Yellow)),
        Span::raw(" Help"),
    ]);

    let block = Block::default().borders(Borders::NONE);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(help, inner);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

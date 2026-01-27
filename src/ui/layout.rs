//! Main layout orchestrating file tree and diff view.

use crate::domain::Diff;
use crate::ui::{diff_view, file_tree, styles};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, ListState, Padding, Paragraph},
    Frame,
};

/// Render the main PR diff view with sidebar and content.
pub fn render_main(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    flat_items: &[file_tree::FlatItem],
    diff_lines: &[diff_view::DiffViewLine],
    selected_tree_item: usize,
    current_file_index: usize,
    scroll: usize,
    branch: &str,
    base: &str,
    tree_state: &mut ListState,
) {
    // Clear with background color
    let bg_block = Block::default().style(styles::style_default());
    frame.render_widget(bg_block, area);

    // Split into sidebar and main content
    let sidebar_width = 35.min(area.width / 3);

    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Length(sidebar_width),
            Constraint::Min(1),
        ])
        .split(area);

    // Render file tree sidebar
    file_tree::render(frame, chunks[0], flat_items, selected_tree_item, tree_state);

    // Render diff view
    diff_view::render(
        frame,
        chunks[1],
        diff,
        diff_lines,
        scroll,
        current_file_index,
        branch,
        base,
    );
}

/// Render help overlay.
pub fn render_help(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(50, 60, area);

    frame.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Keyboard Shortcuts",
            Style::default()
                .fg(styles::FG_DEFAULT)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  j/↓      ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Move down"),
        ]),
        Line::from(vec![
            Span::styled("  k/↑      ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Move up"),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Jump to file"),
        ]),
        Line::from(vec![
            Span::styled("  Tab      ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Toggle sidebar focus"),
        ]),
        Line::from(vec![
            Span::styled("  n        ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Next file"),
        ]),
        Line::from(vec![
            Span::styled("  p        ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Previous file"),
        ]),
        Line::from(vec![
            Span::styled("  g        ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Go to top"),
        ]),
        Line::from(vec![
            Span::styled("  G        ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Go to bottom"),
        ]),
        Line::from(vec![
            Span::styled("  q        ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("  ?        ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Toggle help"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close",
            styles::style_muted(),
        )),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(styles::FG_MUTED))
                .padding(Padding::uniform(1))
                .style(Style::default().bg(styles::BG_SIDEBAR)),
        );

    frame.render_widget(help, popup_area);
}

/// Render an error or empty state.
pub fn render_empty(frame: &mut Frame, area: Rect, message: &str, branch: &str, base: &str) {
    let bg_block = Block::default().style(styles::style_default());
    frame.render_widget(bg_block, area);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {} → {}", branch, base),
            Style::default().fg(styles::FG_DEFAULT),
        )),
        Line::from(""),
        Line::from(Span::styled(format!("  {}", message), styles::style_muted())),
    ];

    let content = Paragraph::new(text).style(styles::style_default());
    frame.render_widget(content, area);
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

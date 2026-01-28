//! Main layout orchestrating file tree and diff view.

use crate::app::DiffSource;
use crate::domain::Diff;
use crate::ui::{diff_view, file_tree, styles};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, ListState, Padding, Paragraph},
    Frame,
};
use std::collections::HashSet;

/// Render the main PR diff view with sidebar and content.
#[allow(clippy::too_many_arguments)]
pub fn render_main(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    flat_items: &[file_tree::FlatItem],
    diff_lines: &[diff_view::DiffViewLine],
    selected_tree_item: usize,
    current_file_index: usize,
    scroll: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    filter: &str,
    filter_focused: bool,
    view_mode: diff_view::DiffViewMode,
    branch: &str,
    base: &str,
    tree_state: &mut ListState,
    sidebar_collapsed: bool,
    has_pending_changes: bool,
    diff_source: DiffSource,
    uncommitted_files: &HashSet<String>,
) {
    // Split into header and main content area (with padding below header)
    let vertical_chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    // Render full-width header
    render_global_header(frame, vertical_chunks[0], diff, branch, base, current_file_index, viewed, sidebar_collapsed, has_pending_changes, diff_source);

    if sidebar_collapsed {
        // Full-width diff view when sidebar is collapsed
        // Add horizontal padding
        let padded_area = Rect {
            x: vertical_chunks[1].x + 2,
            y: vertical_chunks[1].y,
            width: vertical_chunks[1].width.saturating_sub(4),
            height: vertical_chunks[1].height,
        };

        match view_mode {
            diff_view::DiffViewMode::Unified => {
                diff_view::render_unified(
                    frame,
                    padded_area,
                    diff,
                    diff_lines,
                    scroll,
                    current_file_index,
                    collapsed,
                    viewed,
                    diff_source,
                    uncommitted_files,
                );
            }
            diff_view::DiffViewMode::Split => {
                diff_view::render_split(
                    frame,
                    padded_area,
                    diff,
                    diff_lines,
                    scroll,
                    current_file_index,
                    collapsed,
                    viewed,
                    diff_source,
                    uncommitted_files,
                );
            }
        }
    } else {
        // Split main area into sidebar and content
        let sidebar_width = 40.min(vertical_chunks[1].width / 3);

        let horizontal_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
            .split(vertical_chunks[1]);

        // Render file tree sidebar
        file_tree::render(
            frame,
            horizontal_chunks[0],
            flat_items,
            selected_tree_item,
            current_file_index,
            viewed,
            filter,
            filter_focused,
            tree_state,
        );

        // Add horizontal padding to diff area
        let diff_area = Rect {
            x: horizontal_chunks[1].x + 1,
            y: horizontal_chunks[1].y,
            width: horizontal_chunks[1].width.saturating_sub(2),
            height: horizontal_chunks[1].height,
        };

        // Render diff view
        match view_mode {
            diff_view::DiffViewMode::Unified => {
                diff_view::render_unified(
                    frame,
                    diff_area,
                    diff,
                    diff_lines,
                    scroll,
                    current_file_index,
                    collapsed,
                    viewed,
                    diff_source,
                    uncommitted_files,
                );
            }
            diff_view::DiffViewMode::Split => {
                diff_view::render_split(
                    frame,
                    diff_area,
                    diff,
                    diff_lines,
                    scroll,
                    current_file_index,
                    collapsed,
                    viewed,
                    diff_source,
                    uncommitted_files,
                );
            }
        }
    }
}

/// Render the global header spanning full width.
fn render_global_header(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    branch: &str,
    base: &str,
    current_file: usize,
    viewed: &HashSet<usize>,
    sidebar_collapsed: bool,
    has_pending_changes: bool,
    diff_source: DiffSource,
) {
    let stats = diff.total_stats();
    let file_count = diff.file_count();
    let viewed_count = viewed.len();
    let file_indicator = format!("File {}/{}", current_file + 1, file_count);
    let viewed_indicator = format!("{}/{} viewed", viewed_count, file_count);
    let sidebar_hint = if sidebar_collapsed { "show" } else { "hide" };

    // Diff source mode indicator
    let (source_label, source_color) = match diff_source {
        DiffSource::Committed => ("committed", styles::FG_MUTED),
        DiffSource::Uncommitted => ("uncommitted", styles::FG_WARNING),
        DiffSource::All => ("all", styles::FG_HUNK),
    };

    let mut spans = vec![
        Span::styled("  ", Style::default()),
        Span::styled(branch, Style::default().fg(styles::FG_DEFAULT)),
        Span::styled(" → ", Style::default().fg(styles::FG_MUTED)),
        Span::styled(base, Style::default().fg(styles::FG_DEFAULT)),
        Span::styled("  │  ", Style::default().fg(styles::FG_MUTED)),
        Span::styled(format!("+{}", stats.additions), styles::style_stat_addition()),
        Span::styled(" ", Style::default().fg(styles::FG_MUTED)),
        Span::styled(format!("-{}", stats.deletions), styles::style_stat_deletion()),
        Span::styled("  │  ", Style::default().fg(styles::FG_MUTED)),
        Span::styled(file_indicator, Style::default().fg(styles::FG_MUTED)),
        Span::styled("  │  ", Style::default().fg(styles::FG_MUTED)),
        Span::styled(viewed_indicator, Style::default().fg(styles::FG_ADDITION)),
        // Diff source indicator
        Span::styled("  │  ", Style::default().fg(styles::FG_MUTED)),
        Span::styled("u", Style::default().fg(styles::FG_HUNK)),
        Span::styled(" ", Style::default().fg(styles::FG_MUTED)),
        Span::styled(source_label, Style::default().fg(source_color)),
    ];

    // Show refresh indicator if there are pending changes
    if has_pending_changes {
        spans.push(Span::styled("  │  ", Style::default().fg(styles::FG_MUTED)));
        spans.push(Span::styled("● ", Style::default().fg(styles::FG_WARNING)));
        spans.push(Span::styled("r", Style::default().fg(styles::FG_WARNING)));
        spans.push(Span::styled(" refresh", Style::default().fg(styles::FG_WARNING)));
    }

    spans.extend([
        Span::styled("  │  ", Style::default().fg(styles::FG_MUTED)),
        Span::styled("v", Style::default().fg(styles::FG_HUNK)),
        Span::styled(" viewed  ", Style::default().fg(styles::FG_MUTED)),
        Span::styled("b", Style::default().fg(styles::FG_HUNK)),
        Span::styled(format!(" {}  ", sidebar_hint), Style::default().fg(styles::FG_MUTED)),
        Span::styled("n/p", Style::default().fg(styles::FG_HUNK)),
        Span::styled(" nav  ", Style::default().fg(styles::FG_MUTED)),
        Span::styled("s", Style::default().fg(styles::FG_HUNK)),
        Span::styled(" split  ", Style::default().fg(styles::FG_MUTED)),
        Span::styled("?", Style::default().fg(styles::FG_HUNK)),
        Span::styled(" help", Style::default().fg(styles::FG_MUTED)),
    ]);

    let header_line = Line::from(spans);
    let header = Paragraph::new(vec![Line::from(""), header_line]);
    frame.render_widget(header, area);
}

/// Render help overlay.
pub fn render_help(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(55, 70, area);

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
        Line::from(Span::styled("  Navigation", styles::style_muted())),
        Line::from(vec![
            Span::styled("  j/↓       ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Move down"),
        ]),
        Line::from(vec![
            Span::styled("  k/↑       ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Move up"),
        ]),
        Line::from(vec![
            Span::styled("  g/G       ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Go to top/bottom"),
        ]),
        Line::from(vec![
            Span::styled("  n/p       ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Next/previous file"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Actions", styles::style_muted())),
        Line::from(vec![
            Span::styled("  Enter     ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Select file / toggle directory"),
        ]),
        Line::from(vec![
            Span::styled("  Tab       ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Switch pane focus"),
        ]),
        Line::from(vec![
            Span::styled("  /         ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Focus filter input"),
        ]),
        Line::from(vec![
            Span::styled("  v         ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Mark file as viewed"),
        ]),
        Line::from(vec![
            Span::styled("  c         ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Collapse/expand file"),
        ]),
        Line::from(vec![
            Span::styled("  s         ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Toggle split/unified view"),
        ]),
        Line::from(vec![
            Span::styled("  b         ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Toggle sidebar"),
        ]),
        Line::from(vec![
            Span::styled("  r         ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Refresh (reload git changes)"),
        ]),
        Line::from(vec![
            Span::styled("  u         ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Cycle diff source (committed/uncommitted/all)"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  General", styles::style_muted())),
        Line::from(vec![
            Span::styled("  ?         ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Toggle help"),
        ]),
        Line::from(vec![
            Span::styled("  q/Esc     ", Style::default().fg(styles::FG_ADDITION)),
            Span::raw("Quit / close"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close",
            styles::style_muted(),
        )),
    ];

    let help = Paragraph::new(help_text).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(styles::FG_MUTED))
            .padding(Padding::uniform(1))
            .style(Style::default().bg(styles::BG_SIDEBAR)),
    );

    frame.render_widget(help, popup_area);
}

/// Render an empty state.
pub fn render_empty(frame: &mut Frame, area: Rect, message: &str, branch: &str, base: &str) {
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

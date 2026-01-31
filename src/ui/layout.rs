//! Main layout orchestrating file tree and diff view.

use crate::app::{DiffSource, Focus, ViewMode};
use crate::domain::{Comment, Diff};
use crate::keymap::Keymap;
use crate::ui::{diff_view, file_tree, styles};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
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
    cursor: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    stale_viewed: &HashSet<usize>,
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
    comments: &[Comment],
    show_comments: bool,
    visual_selection: Option<(usize, usize)>,
    focused_comment: Option<i64>,
    draft_comment: Option<&(String, usize, usize, String)>, // (file_path, start, end, body)
    reply_info: Option<(i64, &str)>, // (comment_id, input_text) for reply input
    focus: Focus,
    mode: ViewMode,
) {
    // Split into header, main content, and status bar
    let vertical_chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // Header (reduced)
            Constraint::Min(1),     // Content
            Constraint::Length(2),  // Status bar (border + content)
        ])
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
                    cursor,
                    current_file_index,
                    collapsed,
                    viewed,
                    stale_viewed,
                    diff_source,
                    uncommitted_files,
                    comments,
                    show_comments,
                    visual_selection,
                    focused_comment,
                    draft_comment,
                    reply_info,
                );
            }
            diff_view::DiffViewMode::Split => {
                diff_view::render_split(
                    frame,
                    padded_area,
                    diff,
                    diff_lines,
                    scroll,
                    cursor,
                    current_file_index,
                    collapsed,
                    viewed,
                    stale_viewed,
                    diff_source,
                    uncommitted_files,
                    comments,
                    show_comments,
                    visual_selection,
                    focused_comment,
                    draft_comment,
                    reply_info,
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
                    cursor,
                    current_file_index,
                    collapsed,
                    viewed,
                    stale_viewed,
                    diff_source,
                    uncommitted_files,
                    comments,
                    show_comments,
                    visual_selection,
                    focused_comment,
                    draft_comment,
                    reply_info,
                );
            }
            diff_view::DiffViewMode::Split => {
                diff_view::render_split(
                    frame,
                    diff_area,
                    diff,
                    diff_lines,
                    scroll,
                    cursor,
                    current_file_index,
                    collapsed,
                    viewed,
                    stale_viewed,
                    diff_source,
                    uncommitted_files,
                    comments,
                    show_comments,
                    visual_selection,
                    focused_comment,
                    draft_comment,
                    reply_info,
                );
            }
        }
    }

    // Render status bar at the bottom
    render_status_bar(
        frame,
        vertical_chunks[2],
        focus,
        sidebar_collapsed,
        show_comments,
        view_mode,
        filter,
        mode,
        visual_selection,
    );
}

/// Render Zed-style status bar with icon toggles.
fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    _focus: Focus,
    sidebar_collapsed: bool,
    show_comments: bool,
    view_mode: diff_view::DiffViewMode,
    filter: &str,
    mode: ViewMode,
    visual_selection: Option<(usize, usize)>,
) {
    let mut left_spans = Vec::new();
    let mut center_spans = Vec::new();
    let mut right_spans = Vec::new();

    // Left side: toggle icons
    // Files/sidebar toggle (b)
    add_toggle_icon(
        &mut left_spans,
        "≡",
        "Files",
        "b",
        !sidebar_collapsed,
    );

    left_spans.push(Span::styled(" │ ", Style::default().fg(styles::FG_BORDER)));

    // Split/unified view toggle (s)
    add_toggle_icon(
        &mut left_spans,
        "◫",
        "Split",
        "s",
        view_mode == diff_view::DiffViewMode::Split,
    );

    // Comments toggle (C)
    add_toggle_icon(
        &mut left_spans,
        "◇",
        "Comments",
        "C",
        show_comments,
    );

    left_spans.push(Span::styled(" │ ", Style::default().fg(styles::FG_BORDER)));

    // Filter indicator
    let filter_active = !filter.is_empty();
    add_toggle_icon(
        &mut left_spans,
        "⌕",
        if filter_active { filter } else { "Filter" },
        "/",
        filter_active,
    );

    // Center: Visual/Comment mode indicator (minimal)
    if mode == ViewMode::Visual || mode == ViewMode::CommentInput {
        let selection_info = visual_selection
            .map(|(start, end)| {
                if start == end {
                    format!("L{}", start + 1)
                } else {
                    format!("L{}:{}", start + 1, end + 1)
                }
            })
            .unwrap_or_default();

        let (icon, hint) = if mode == ViewMode::CommentInput {
            ("✎", "Enter ⏎  Esc ✗")
        } else {
            ("▋", "c comment  Esc ✗")
        };

        center_spans.push(Span::styled(
            format!(" {} ", icon),
            Style::default().fg(styles::FG_HUNK),
        ));
        center_spans.push(Span::styled(
            format!("[{}]", selection_info),
            Style::default().fg(styles::FG_DEFAULT),
        ));
        center_spans.push(Span::styled(
            format!("  {}", hint),
            Style::default().fg(styles::FG_MUTED),
        ));
    }

    // Right side: help and quit
    right_spans.push(Span::styled("?", Style::default().fg(styles::FG_MUTED)));
    right_spans.push(Span::styled(" Help ", Style::default().fg(styles::FG_MUTED)));
    right_spans.push(Span::styled("q", Style::default().fg(styles::FG_MUTED)));
    right_spans.push(Span::styled(" Quit ", Style::default().fg(styles::FG_MUTED)));

    // Calculate padding to center the center_spans
    let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
    let center_width: usize = center_spans.iter().map(|s| s.content.chars().count()).sum();
    let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();

    let total_content = left_width + center_width + right_width + 2; // +2 for edge padding
    let remaining = (area.width as usize).saturating_sub(total_content);

    // Split remaining space: more on left of center, rest on right
    let left_pad = remaining / 2;
    let right_pad = remaining.saturating_sub(left_pad);

    let mut spans = vec![Span::raw(" ")];
    spans.extend(left_spans);
    spans.push(Span::raw(" ".repeat(left_pad)));
    spans.extend(center_spans);
    spans.push(Span::raw(" ".repeat(right_pad)));
    spans.extend(right_spans);
    spans.push(Span::raw(" "));

    // Top border line
    let border_line = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(styles::FG_BORDER),
    ));

    let status_line = Line::from(spans);
    let para = Paragraph::new(vec![border_line, status_line]);
    frame.render_widget(para, area);
}

/// Add a toggle icon with label and hotkey.
fn add_toggle_icon(spans: &mut Vec<Span<'static>>, icon: &'static str, label: &str, key: &'static str, active: bool) {
    let fg = if active { styles::FG_DEFAULT } else { styles::FG_MUTED };

    spans.push(Span::styled(format!(" {}", icon), Style::default().fg(fg)));
    spans.push(Span::styled(format!(" {}", label), Style::default().fg(fg)));
    spans.push(Span::styled(format!(" {}", key), Style::default().fg(styles::FG_BORDER)));
    spans.push(Span::raw(" "));
}

/// Render comment input overlay.
pub fn render_comment_input(
    frame: &mut Frame,
    area: Rect,
    comment_text: &str,
    selection: Option<(usize, usize)>,
) {
    let popup_area = centered_rect(60, 30, area);

    frame.render_widget(Clear, popup_area);

    let selection_info = selection
        .map(|(start, end)| {
            if start == end {
                format!("Line {}", start + 1)
            } else {
                format!("Lines {}-{}", start + 1, end + 1)
            }
        })
        .unwrap_or_default();

    let title = format!(" Add comment ({}) ", selection_info);

    let input_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(comment_text, Style::default().fg(styles::FG_DEFAULT)),
            Span::styled("█", Style::default().fg(styles::FG_HUNK)), // Cursor
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Enter to submit, Esc to cancel",
            styles::style_muted(),
        )),
    ];

    let input = Paragraph::new(input_lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(styles::FG_HUNK))
            .padding(Padding::uniform(1))
            .style(Style::default().bg(styles::BG_SIDEBAR)),
    );

    frame.render_widget(input, popup_area);
}

/// Render the global header spanning full width.
fn render_global_header(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    branch: &str,
    base: &str,
    _current_file: usize,
    viewed: &HashSet<usize>,
    _sidebar_collapsed: bool,
    has_pending_changes: bool,
    diff_source: DiffSource,
) {
    let stats = diff.total_stats();
    let file_count = diff.file_count();
    let viewed_count = viewed.len();

    // Left side: title and summary
    let mut left_spans = vec![
        Span::styled(" ", Style::default()),
        Span::styled(branch, Style::default().fg(styles::FG_PATH).add_modifier(Modifier::BOLD)),
        Span::styled(" → ", Style::default().fg(styles::FG_MUTED)),
        Span::styled(base, Style::default().fg(styles::FG_MUTED)),
        Span::styled("  ", Style::default()),
        Span::styled(format!("{} files", file_count), Style::default().fg(styles::FG_MUTED)),
        Span::styled("  ", Style::default()),
        Span::styled(format!("+{}", stats.additions), styles::style_stat_addition()),
        Span::styled(" ", Style::default()),
        Span::styled(format!("-{}", stats.deletions), styles::style_stat_deletion()),
        Span::styled("  ", Style::default()),
        Span::styled("✓", Style::default().fg(styles::FG_ADDITION)),
        Span::styled(format!(" {}/{}", viewed_count, file_count), Style::default().fg(styles::FG_MUTED)),
    ];

    // Show refresh indicator if there are pending changes
    if has_pending_changes {
        left_spans.push(Span::styled("  ", Style::default()));
        left_spans.push(Span::styled("●", Style::default().fg(styles::FG_WARNING)));
        left_spans.push(Span::styled(" changed", Style::default().fg(styles::FG_WARNING)));
    }

    // Right side: diff source toggle
    let mut right_spans = Vec::new();

    // Display mode selector
    right_spans.push(Span::styled("u ", Style::default().fg(styles::FG_BORDER)));

    let sources = [
        ("Committed", DiffSource::Committed),
        ("Staged", DiffSource::Uncommitted),
        ("All", DiffSource::All),
    ];

    for (i, (label, source)) in sources.iter().enumerate() {
        let is_active = diff_source == *source;
        let style = if is_active {
            Style::default().fg(styles::FG_DEFAULT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(styles::FG_MUTED)
        };

        if i > 0 {
            right_spans.push(Span::styled(" │ ", Style::default().fg(styles::FG_BORDER)));
        }
        right_spans.push(Span::styled(*label, style));
    }

    right_spans.push(Span::styled(" ", Style::default()));

    // Calculate padding
    let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
    let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
    let padding = (area.width as usize).saturating_sub(left_width + right_width);

    let mut spans = left_spans;
    spans.push(Span::raw(" ".repeat(padding)));
    spans.extend(right_spans);

    // Header line with bottom border
    let header_line = Line::from(spans);
    let border_line = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(styles::FG_BORDER),
    ));

    let header = Paragraph::new(vec![header_line, border_line]);
    frame.render_widget(header, area);
}

/// Render help overlay using keymap data.
pub fn render_help(frame: &mut Frame, area: Rect, keymap: &Keymap) {
    let popup_area = centered_rect(55, 70, area);

    frame.render_widget(Clear, popup_area);

    let mut help_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Keyboard Shortcuts",
            Style::default()
                .fg(styles::FG_DEFAULT)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Generate help from keymap
    for (category, entries) in keymap.help_entries() {
        help_text.push(Line::from(Span::styled(
            format!("  {}", category.display_name()),
            styles::style_muted(),
        )));

        for entry in entries {
            let key_col = if let Some(hint) = entry.context_hint {
                if hint.is_empty() {
                    format!("  {:<10}", entry.key_display)
                } else {
                    format!("  {} ({})", entry.key_display, hint)
                }
            } else {
                format!("  {:<10}", entry.key_display)
            };

            // Pad to fixed width for alignment
            let key_col = format!("{:<14}", key_col);

            help_text.push(Line::from(vec![
                Span::styled(key_col, Style::default().fg(styles::FG_ADDITION)),
                Span::raw(entry.description),
            ]));
        }

        help_text.push(Line::from(""));
    }

    help_text.push(Line::from(Span::styled(
        "  Press any key to close",
        styles::style_muted(),
    )));

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

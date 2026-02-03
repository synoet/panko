//! Main layout orchestrating file tree and diff view.

use crate::app::{DiffSource, Focus, ViewMode};
use crate::domain::{Comment, Diff};
use crate::keymap::Keymap;
use crate::ui::{diff_view, file_tree, styles};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
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
    // Fill the full background so theme colors apply consistently.
    frame.render_widget(
        Block::default().style(Style::default().bg(styles::bg_default())),
        area,
    );

    // Split into header and main content
    let vertical_chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // Header
            Constraint::Min(1),     // Content
        ])
        .split(area);

    // Render full-width header
    render_global_header(frame, vertical_chunks[0], diff, branch, base, current_file_index, viewed, sidebar_collapsed, has_pending_changes, diff_source);

    if sidebar_collapsed {
        // Full-width diff view when sidebar is collapsed
        // Split to include panel bottom bar
        let diff_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(2)])
            .split(vertical_chunks[1]);

        // Add horizontal padding
        let padded_area = Rect {
            x: diff_chunks[0].x + 2,
            y: diff_chunks[0].y,
            width: diff_chunks[0].width.saturating_sub(4),
            height: diff_chunks[0].height,
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

        // Render diff hints bar at bottom
        render_diff_hints(frame, diff_chunks[1], true, view_mode, show_comments, mode, visual_selection);
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
            focus == Focus::FileTree,
        );

        // Split diff area to include panel bottom bar
        let diff_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(2)])
            .split(horizontal_chunks[1]);

        // Add horizontal padding to diff area
        let diff_area = Rect {
            x: diff_chunks[0].x + 1,
            y: diff_chunks[0].y,
            width: diff_chunks[0].width.saturating_sub(2),
            height: diff_chunks[0].height,
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

        // Render diff hints bar at bottom
        render_diff_hints(frame, diff_chunks[1], focus == Focus::DiffView, view_mode, show_comments, mode, visual_selection);
    }
}

/// Render panel bottom bar for the diff view.
#[allow(clippy::too_many_arguments)]
fn render_diff_hints(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    view_mode: diff_view::DiffViewMode,
    show_comments: bool,
    mode: ViewMode,
    visual_selection: Option<(usize, usize)>,
) {
    let border_color = if focused {
        styles::fg_hunk()
    } else {
        styles::fg_border()
    };
    let hint_style = Style::default().fg(styles::fg_muted());
    let key_style = Style::default().fg(if focused { styles::fg_default() } else { styles::fg_muted() });
    let focus_style = if focused {
        Style::default().fg(styles::fg_hunk()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(styles::fg_muted())
    };

    // Split toggle
    let split_active = view_mode == diff_view::DiffViewMode::Split;
    let split_fg = if split_active { styles::fg_default() } else { styles::fg_muted() };

    // Comments toggle
    let comments_fg = if show_comments { styles::fg_default() } else { styles::fg_muted() };

    // Build left spans
    let mut left_spans = vec![
        Span::styled(" 2", focus_style),
        Span::styled(" │ ", Style::default().fg(styles::fg_border())),
        Span::styled("◫", Style::default().fg(split_fg)),
        Span::styled(" Split ", Style::default().fg(split_fg)),
        Span::styled("s", Style::default().fg(styles::fg_border())),
        Span::styled(" │ ", Style::default().fg(styles::fg_border())),
        Span::styled("◇", Style::default().fg(comments_fg)),
        Span::styled(" Comments ", Style::default().fg(comments_fg)),
        Span::styled("C", Style::default().fg(styles::fg_border())),
    ];

    // Show visual mode indicator if active
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

        left_spans.push(Span::styled(" │ ", Style::default().fg(styles::fg_border())));
        left_spans.push(Span::styled(format!("{} ", icon), Style::default().fg(styles::fg_hunk())));
        left_spans.push(Span::styled(format!("[{}] ", selection_info), Style::default().fg(styles::fg_default())));
        left_spans.push(Span::styled(hint, Style::default().fg(styles::fg_muted())));
    } else {
        // Normal hints when not in visual mode
        left_spans.push(Span::styled(" │ ", Style::default().fg(styles::fg_border())));
        left_spans.push(Span::styled("/", key_style));
        left_spans.push(Span::styled(" Search ", hint_style));
        left_spans.push(Span::styled("v", key_style));
        left_spans.push(Span::styled(" Select ", hint_style));
        left_spans.push(Span::styled("n/p", key_style));
        left_spans.push(Span::styled(" File ", hint_style));
    }

    // Right side: help and quit
    let right_spans = vec![
        Span::styled("?", Style::default().fg(styles::fg_border())),
        Span::styled(" Help ", hint_style),
        Span::styled("q", Style::default().fg(styles::fg_border())),
        Span::styled(" Quit ", hint_style),
    ];

    // Calculate padding
    let left_width: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
    let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
    let padding = (area.width as usize).saturating_sub(left_width + right_width);

    let mut spans = left_spans;
    spans.push(Span::raw(" ".repeat(padding)));
    spans.extend(right_spans);

    let hints = Line::from(spans);

    // Top border line (file tree's ┤ handles the left connection when sidebar visible)
    let border_line = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(border_color).bg(styles::bg_header()),
    ));

    let hints_widget = Paragraph::new(vec![border_line, hints])
        .style(Style::default().bg(styles::bg_header()));
    frame.render_widget(hints_widget, area);
}

/// Add a toggle icon with label and hotkey.
/// Render the global header spanning full width.
#[allow(clippy::too_many_arguments)]
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
        Span::styled(branch, Style::default().fg(styles::fg_path()).add_modifier(Modifier::BOLD)),
        Span::styled(" → ", Style::default().fg(styles::fg_muted())),
        Span::styled(base, Style::default().fg(styles::fg_muted())),
        Span::styled("  ", Style::default()),
        Span::styled(format!("{} files", file_count), Style::default().fg(styles::fg_muted())),
        Span::styled("  ", Style::default()),
        Span::styled(format!("+{}", stats.additions), styles::style_stat_addition()),
        Span::styled(" ", Style::default()),
        Span::styled(format!("-{}", stats.deletions), styles::style_stat_deletion()),
        Span::styled("  ", Style::default()),
        Span::styled("✓", Style::default().fg(styles::fg_addition())),
        Span::styled(format!(" {}/{}", viewed_count, file_count), Style::default().fg(styles::fg_muted())),
    ];

    // Show refresh indicator if there are pending changes
    if has_pending_changes {
        left_spans.push(Span::styled("  ", Style::default()));
        left_spans.push(Span::styled("●", Style::default().fg(styles::fg_warning())));
        left_spans.push(Span::styled(" changed", Style::default().fg(styles::fg_warning())));
    }

    // Right side: diff source toggle
    let mut right_spans = Vec::new();

    // Display mode selector
    right_spans.push(Span::styled("u ", Style::default().fg(styles::fg_border())));

    let sources = [
        ("Committed", DiffSource::Committed),
        ("Staged", DiffSource::Uncommitted),
        ("All", DiffSource::All),
    ];

    for (i, (label, source)) in sources.iter().enumerate() {
        let is_active = diff_source == *source;
        let style = if is_active {
            Style::default().fg(styles::fg_default()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(styles::fg_muted())
        };

        if i > 0 {
            right_spans.push(Span::styled(" │ ", Style::default().fg(styles::fg_border())));
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
        Style::default().fg(styles::fg_border()).bg(styles::bg_header()),
    ));

    let header = Paragraph::new(vec![header_line, border_line])
        .style(Style::default().bg(styles::bg_header()));
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
                .fg(styles::fg_default())
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
                Span::styled(key_col, Style::default().fg(styles::fg_addition())),
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
            .border_style(Style::default().fg(styles::fg_muted()))
            .padding(Padding::uniform(1))
            .style(Style::default().bg(styles::bg_sidebar())),
    );

    frame.render_widget(help, popup_area);
}

/// Render the theme picker overlay.
pub fn render_theme_picker(
    frame: &mut Frame,
    area: Rect,
    themes: &[String],
    selected: usize,
) {
    let popup_area = centered_rect(45, 50, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Themes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(styles::fg_muted()))
        .style(Style::default().bg(styles::bg_sidebar()));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let items: Vec<ListItem> = themes
        .iter()
        .map(|name| {
            ListItem::new(Line::from(Span::styled(
                format!(" {}", name),
                Style::default().fg(styles::fg_default()),
            )))
        })
        .collect();

    let list = List::new(items)
        .style(Style::default().bg(styles::bg_sidebar()))
        .highlight_style(
            Style::default()
                .bg(styles::bg_selected())
                .fg(styles::fg_default())
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    if !themes.is_empty() {
        state.select(Some(selected.min(themes.len() - 1)));
    }

    frame.render_stateful_widget(list, chunks[0], &mut state);

    let hint_area = chunks[1];
    let hint = Paragraph::new(Line::from(Span::styled(
        " Enter apply │ Esc cancel",
        styles::style_muted(),
    )));
    frame.render_widget(hint, hint_area);
}

/// Render the fuzzy search as a bottom drawer above the diff bottom bar.
pub fn render_fuzzy_search(
    frame: &mut Frame,
    area: Rect,
    state: &crate::search::FuzzySearchState,
    sidebar_collapsed: bool,
) {
    // Calculate diff area position
    // Layout: header (2) + content + bottom bar (2, but drawer goes above it)
    let header_height = 2;
    let bottom_bar_height = 2;
    let content_height = area.height.saturating_sub(header_height + bottom_bar_height);

    // Sidebar width when visible
    let sidebar_width = if sidebar_collapsed { 0 } else { 40.min(area.width / 3) };

    // Drawer dimensions - positioned in diff area, above the bottom bar
    let drawer_height = 10.min(content_height);
    let drawer_area = Rect {
        x: area.x + sidebar_width,
        y: area.y + header_height + content_height - drawer_height,
        width: area.width.saturating_sub(sidebar_width),
        height: drawer_height,
    };

    frame.render_widget(Clear, drawer_area);

    // Top border
    let border_line = Line::from(Span::styled(
        "─".repeat(drawer_area.width as usize),
        Style::default().fg(styles::fg_hunk()),
    ));

    // Split drawer into border, input, results, and hints
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top border
            Constraint::Length(1), // Input line
            Constraint::Min(1),    // Results
            Constraint::Length(1), // Hints
        ])
        .split(drawer_area);

    // Render top border
    let border = Paragraph::new(border_line)
        .style(Style::default().bg(styles::bg_sidebar()));
    frame.render_widget(border, chunks[0]);

    // Input line with search icon and query
    let result_count = if state.results.is_empty() {
        String::new()
    } else {
        format!("{}/{}", state.selected_index + 1, state.results.len())
    };

    let input_spans = if state.query.is_empty() {
        vec![
            Span::styled(" /", Style::default().fg(styles::fg_hunk())),
            Span::styled(" █", Style::default().fg(styles::fg_hunk())),
            Span::styled(" Search diff...", Style::default().fg(styles::fg_muted())),
        ]
    } else {
        vec![
            Span::styled(" /", Style::default().fg(styles::fg_hunk())),
            Span::styled(format!(" {}", &state.query), Style::default().fg(styles::fg_default())),
            Span::styled("█", Style::default().fg(styles::fg_hunk())),
            Span::styled(format!("  {}", result_count), Style::default().fg(styles::fg_muted())),
        ]
    };

    let input = Paragraph::new(Line::from(input_spans))
        .style(Style::default().bg(styles::bg_sidebar()));
    frame.render_widget(input, chunks[1]);

    // Results list
    let visible_height = chunks[2].height as usize;
    let results_to_show: Vec<ListItem> = state
        .results
        .iter()
        .skip(state.scroll)
        .take(visible_height)
        .enumerate()
        .map(|(i, result)| {
            let absolute_idx = state.scroll + i;
            let is_selected = absolute_idx == state.selected_index;

            let line_num = result.entry.line_number
                .map(|n| format!(":{}", n))
                .unwrap_or_default();

            let kind_char = match result.entry.line_kind {
                crate::ui::diff_view::LineKind::Addition => "+",
                crate::ui::diff_view::LineKind::Deletion => "-",
                crate::ui::diff_view::LineKind::FileHeader => "F",
                crate::ui::diff_view::LineKind::HunkHeader => "@",
                _ => " ",
            };

            let kind_color = match result.entry.line_kind {
                crate::ui::diff_view::LineKind::Addition => styles::fg_addition(),
                crate::ui::diff_view::LineKind::Deletion => styles::fg_deletion(),
                crate::ui::diff_view::LineKind::FileHeader => styles::fg_path(),
                crate::ui::diff_view::LineKind::HunkHeader => styles::fg_hunk(),
                _ => styles::fg_muted(),
            };

            let file_path = &result.entry.file_path;
            let file_display: String = if file_path.len() > 25 {
                format!("...{}", &file_path[file_path.len().saturating_sub(22)..])
            } else {
                file_path.clone()
            };

            let max_content = (chunks[2].width as usize).saturating_sub(file_display.len() + line_num.len() + 6);
            let content: String = result.entry.content
                .chars()
                .take(max_content)
                .collect();

            let style = if is_selected {
                Style::default()
                    .bg(styles::bg_selected())
                    .fg(styles::fg_default())
            } else {
                Style::default().bg(styles::bg_sidebar())
            };

            ListItem::new(Line::from(vec![
                Span::styled(" ", style),
                Span::styled(kind_char, style.fg(kind_color)),
                Span::styled(" ", style),
                Span::styled(file_display, style.fg(styles::fg_path())),
                Span::styled(line_num, style.fg(styles::fg_line_num())),
                Span::styled("  ", style),
                Span::styled(content, style.fg(styles::fg_default())),
            ]))
        })
        .collect();

    let results_widget = if state.query.is_empty() {
        let placeholder = vec![ListItem::new(Line::from(Span::styled(
            "  Type to search through diff content",
            Style::default().fg(styles::fg_muted()),
        )))];
        List::new(placeholder)
    } else if results_to_show.is_empty() {
        let no_results = vec![ListItem::new(Line::from(Span::styled(
            "  No matches found",
            Style::default().fg(styles::fg_muted()),
        )))];
        List::new(no_results)
    } else {
        List::new(results_to_show)
    };

    let results_list = results_widget.style(Style::default().bg(styles::bg_sidebar()));
    frame.render_widget(results_list, chunks[2]);

    // Hints at bottom
    let hint = Paragraph::new(Line::from(vec![
        Span::styled(" Enter", Style::default().fg(styles::fg_border())),
        Span::styled(" select ", Style::default().fg(styles::fg_muted())),
        Span::styled("Esc", Style::default().fg(styles::fg_border())),
        Span::styled(" close ", Style::default().fg(styles::fg_muted())),
        Span::styled("j/k", Style::default().fg(styles::fg_border())),
        Span::styled(" preview", Style::default().fg(styles::fg_muted())),
    ]))
    .style(Style::default().bg(styles::bg_sidebar()));
    frame.render_widget(hint, chunks[3]);
}

/// Render an empty state.
pub fn render_empty(frame: &mut Frame, area: Rect, message: &str, branch: &str, base: &str) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {} → {}", branch, base),
            Style::default().fg(styles::fg_default()),
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

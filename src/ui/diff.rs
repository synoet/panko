//! Pure render functions for diff views.

use crate::domain::{Diff, DiffLine, FileDiff};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

/// Render the full diff view (PR-style).
pub fn render_full_diff(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    branch: &str,
    base: &str,
    scroll: usize,
    file_index: usize,
) {
    let title = format!(" Changes: {} vs {} (merge-base) ", branch, base);

    let stats = diff.total_stats();
    let subtitle = format!(
        "{} files changed, {} insertions(+), {} deletions(-)",
        diff.file_count(),
        stats.additions,
        stats.deletions
    );

    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    // Header
    let header_block = Block::default()
        .title(title)
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT);
    let header_inner = header_block.inner(chunks[0]);
    frame.render_widget(header_block, chunks[0]);
    frame.render_widget(
        Paragraph::new(subtitle).style(Style::default().fg(Color::DarkGray)),
        header_inner,
    );

    // Diff content
    let lines = build_diff_lines(diff, file_index);
    let visible_lines: Vec<ListItem> = lines
        .iter()
        .skip(scroll)
        .map(|line| ListItem::new(line.clone()))
        .collect();

    let diff_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .padding(Padding::horizontal(1));

    let list = List::new(visible_lines).block(diff_block);
    frame.render_widget(list, chunks[1]);

    // Help bar
    render_diff_help(frame, chunks[2]);
}

/// Render commit detail view.
pub fn render_commit_detail(
    frame: &mut Frame,
    area: Rect,
    commit: &crate::domain::Commit,
    diff: &Diff,
    selected_file: usize,
    list_state: &mut ListState,
) {
    let title = format!(" {}: {} ", commit.short_hash, truncate(commit.summary(), 50));

    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    // Header with commit info
    let header_block = Block::default()
        .title(title)
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT);
    let header_inner = header_block.inner(chunks[0]);
    frame.render_widget(header_block, chunks[0]);

    let info = vec![
        Line::from(vec![
            Span::styled("Author: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{} <{}>", commit.author, commit.email)),
        ]),
        Line::from(vec![
            Span::styled("Date:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(commit.relative_time()),
        ]),
    ];
    frame.render_widget(Paragraph::new(info), header_inner);

    // File list
    let files: Vec<ListItem> = diff
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let style = if i == selected_file {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::raw(&f.path),
                Span::raw("  "),
                Span::styled(
                    format!("+{}", f.stats.additions),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("-{}", f.stats.deletions),
                    Style::default().fg(Color::Red),
                ),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    list_state.select(Some(selected_file));

    let files_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .padding(Padding::horizontal(1));

    let list = List::new(files).block(files_block).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, chunks[1], list_state);

    // Help bar
    render_commit_help(frame, chunks[2]);
}

/// Render file diff view (single file).
pub fn render_file_diff(
    frame: &mut Frame,
    area: Rect,
    file: &FileDiff,
    scroll: usize,
) {
    let title = format!(" {} ", file.path);

    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let lines = build_file_diff_lines(file);
    let visible_lines: Vec<ListItem> = lines
        .iter()
        .skip(scroll)
        .map(|line| ListItem::new(line.clone()))
        .collect();

    let diff_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1));

    let list = List::new(visible_lines).block(diff_block);
    frame.render_widget(list, chunks[0]);

    render_file_help(frame, chunks[1]);
}

fn build_diff_lines(diff: &Diff, highlight_file: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    for (file_idx, file) in diff.files.iter().enumerate() {
        // File header
        let file_style = if file_idx == highlight_file {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        lines.push(Line::from(Span::styled(
            file.path.clone(),
            file_style,
        )));
        lines.push(Line::from(Span::styled(
            "─".repeat(file.path.len()),
            Style::default().fg(Color::DarkGray),
        )));

        if file.is_binary {
            lines.push(Line::from(Span::styled(
                "Binary file",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for hunk in &file.hunks {
                lines.push(Line::from(Span::styled(
                    hunk.header(),
                    Style::default().fg(Color::Magenta),
                )));

                for diff_line in &hunk.lines {
                    lines.push(render_diff_line(diff_line));
                }
            }
        }

        lines.push(Line::from(""));
    }

    lines
}

fn build_file_diff_lines(file: &FileDiff) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if file.is_binary {
        lines.push(Line::from(Span::styled(
            "Binary file",
            Style::default().fg(Color::DarkGray),
        )));
        return lines;
    }

    for hunk in &file.hunks {
        lines.push(Line::from(Span::styled(
            hunk.header(),
            Style::default().fg(Color::Magenta),
        )));

        for diff_line in &hunk.lines {
            lines.push(render_diff_line(diff_line));
        }

        lines.push(Line::from(""));
    }

    lines
}

fn render_diff_line(line: &DiffLine) -> Line<'static> {
    let (prefix, content, style) = match line {
        DiffLine::Addition(s) => ('+', s.clone(), Style::default().fg(Color::Green)),
        DiffLine::Deletion(s) => ('-', s.clone(), Style::default().fg(Color::Red)),
        DiffLine::Context(s) => (' ', s.clone(), Style::default()),
    };

    Line::from(Span::styled(format!("{}{}", prefix, content), style))
}

fn render_diff_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled("[j/k]", Style::default().fg(Color::Yellow)),
        Span::raw(" Scroll  "),
        Span::styled("[n/p]", Style::default().fg(Color::Yellow)),
        Span::raw(" Next/prev file  "),
        Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
        Span::raw(" Back"),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(help, inner);
}

fn render_commit_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(Color::Yellow)),
        Span::raw(" View file diff  "),
        Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
        Span::raw(" Back"),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(help, inner);
}

fn render_file_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled("[j/k]", Style::default().fg(Color::Yellow)),
        Span::raw(" Scroll  "),
        Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
        Span::raw(" Back"),
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

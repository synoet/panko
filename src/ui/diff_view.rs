//! Main diff view component showing GitHub-style diff.

use crate::domain::{Diff, DiffLine};
use crate::ui::styles;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

/// A single rendered line in the diff view.
#[derive(Debug, Clone)]
pub struct DiffViewLine {
    pub old_line_num: Option<u32>,
    pub new_line_num: Option<u32>,
    pub kind: LineKind,
    pub content: String,
    pub file_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    FileHeader,
    HunkHeader,
    Context,
    Addition,
    Deletion,
    Empty,
}

/// Build all lines for the diff view.
pub fn build_diff_lines(diff: &Diff) -> Vec<DiffViewLine> {
    let mut lines = Vec::new();

    for (file_idx, file) in diff.files.iter().enumerate() {
        // File header
        lines.push(DiffViewLine {
            old_line_num: None,
            new_line_num: None,
            kind: LineKind::FileHeader,
            content: file.path.clone(),
            file_index: file_idx,
        });

        if file.is_binary {
            lines.push(DiffViewLine {
                old_line_num: None,
                new_line_num: None,
                kind: LineKind::Empty,
                content: "Binary file".to_string(),
                file_index: file_idx,
            });
        } else {
            for hunk in &file.hunks {
                // Hunk header
                lines.push(DiffViewLine {
                    old_line_num: None,
                    new_line_num: None,
                    kind: LineKind::HunkHeader,
                    content: hunk.header(),
                    file_index: file_idx,
                });

                let mut old_num = hunk.old_start;
                let mut new_num = hunk.new_start;

                for diff_line in &hunk.lines {
                    match diff_line {
                        DiffLine::Context(content) => {
                            lines.push(DiffViewLine {
                                old_line_num: Some(old_num),
                                new_line_num: Some(new_num),
                                kind: LineKind::Context,
                                content: content.clone(),
                                file_index: file_idx,
                            });
                            old_num += 1;
                            new_num += 1;
                        }
                        DiffLine::Addition(content) => {
                            lines.push(DiffViewLine {
                                old_line_num: None,
                                new_line_num: Some(new_num),
                                kind: LineKind::Addition,
                                content: content.clone(),
                                file_index: file_idx,
                            });
                            new_num += 1;
                        }
                        DiffLine::Deletion(content) => {
                            lines.push(DiffViewLine {
                                old_line_num: Some(old_num),
                                new_line_num: None,
                                kind: LineKind::Deletion,
                                content: content.clone(),
                                file_index: file_idx,
                            });
                            old_num += 1;
                        }
                    }
                }
            }
        }

        // Empty line between files
        lines.push(DiffViewLine {
            old_line_num: None,
            new_line_num: None,
            kind: LineKind::Empty,
            content: String::new(),
            file_index: file_idx,
        });
    }

    lines
}

/// Find the line index where a file starts.
pub fn find_file_start(lines: &[DiffViewLine], file_index: usize) -> usize {
    lines
        .iter()
        .position(|l| l.file_index == file_index && l.kind == LineKind::FileHeader)
        .unwrap_or(0)
}

/// Render the diff view.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    lines: &[DiffViewLine],
    scroll: usize,
    current_file: usize,
    branch: &str,
    base: &str,
) {
    // Header showing branch info
    let header_height = 2;
    let content_height = area.height.saturating_sub(header_height);

    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(1),
        ])
        .split(area);

    // Header
    let stats = diff.total_stats();
    let header_text = vec![
        Line::from(vec![
            Span::styled(
                format!(" {} ", branch),
                Style::default().fg(styles::FG_DEFAULT),
            ),
            Span::styled("→ ", styles::style_muted()),
            Span::styled(
                format!("{} ", base),
                Style::default().fg(styles::FG_DEFAULT),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("+{}", stats.additions),
                styles::style_stat_addition(),
            ),
            Span::raw(" "),
            Span::styled(
                format!("-{}", stats.deletions),
                styles::style_stat_deletion(),
            ),
            Span::styled(
                format!("  {} files", diff.file_count()),
                styles::style_muted(),
            ),
        ]),
    ];

    let header = Paragraph::new(header_text)
        .style(styles::style_default())
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(styles::FG_MUTED)));
    frame.render_widget(header, chunks[0]);

    // Diff content
    let visible_height = content_height as usize;
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(scroll)
        .take(visible_height)
        .map(|line| render_line(line, current_file))
        .collect();

    let diff_content = Paragraph::new(visible_lines).style(styles::style_default());
    frame.render_widget(diff_content, chunks[1]);

    // Scrollbar
    if lines.len() > visible_height {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let mut scrollbar_state = ScrollbarState::new(lines.len())
            .position(scroll);
        frame.render_stateful_widget(
            scrollbar,
            chunks[1],
            &mut scrollbar_state,
        );
    }
}

fn render_line(line: &DiffViewLine, current_file: usize) -> Line<'static> {
    match line.kind {
        LineKind::FileHeader => {
            let is_current = line.file_index == current_file;
            let style = if is_current {
                styles::style_file_header().add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                styles::style_file_header()
            };
            Line::from(vec![
                Span::styled("        ", style),
                Span::styled(format!("  {} ", line.content), style),
            ])
        }
        LineKind::HunkHeader => {
            Line::from(vec![
                Span::styled("        ", styles::style_hunk_header()),
                Span::styled(format!(" {} ", line.content), styles::style_hunk_header()),
            ])
        }
        LineKind::Context => {
            let old_num = format_line_num(line.old_line_num);
            let new_num = format_line_num(line.new_line_num);
            Line::from(vec![
                Span::styled(old_num, styles::style_line_num()),
                Span::styled(new_num, styles::style_line_num()),
                Span::styled(format!("  {}", line.content), styles::style_context()),
            ])
        }
        LineKind::Addition => {
            let old_num = "    ";
            let new_num = format_line_num(line.new_line_num);
            Line::from(vec![
                Span::styled(old_num, styles::style_addition_line_num()),
                Span::styled(new_num, styles::style_addition_line_num()),
                Span::styled(format!("+ {}", line.content), styles::style_addition()),
            ])
        }
        LineKind::Deletion => {
            let old_num = format_line_num(line.old_line_num);
            let new_num = "    ";
            Line::from(vec![
                Span::styled(old_num, styles::style_deletion_line_num()),
                Span::styled(new_num, styles::style_deletion_line_num()),
                Span::styled(format!("- {}", line.content), styles::style_deletion()),
            ])
        }
        LineKind::Empty => {
            Line::from(vec![Span::raw("")])
        }
    }
}

fn format_line_num(num: Option<u32>) -> String {
    match num {
        Some(n) => format!("{:>4}", n),
        None => "    ".to_string(),
    }
}

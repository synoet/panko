//! Main diff view component with GitHub-style file blocks, word-level diffs, and split view.
//! Performance optimized: syntax highlighting is cached during build, not render.

#![allow(dead_code)]

use crate::app::DiffSource;
use crate::domain::{Comment, Diff, DiffLine, DiffStats};
use crate::ui::{styles, syntax};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use std::collections::HashSet;

/// View mode for diffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffViewMode {
    #[default]
    Unified,
    Split,
}

/// Pre-computed highlighted segment (cached from syntax highlighting).
#[derive(Debug, Clone)]
pub struct HighlightedSegment {
    pub text: String,
    pub fg: Color,
    pub bold: bool,
    pub italic: bool,
    pub is_changed: bool, // For word-level diff overlay
}

/// A rendered line in the diff view.
#[derive(Debug, Clone)]
pub struct DiffViewLine {
    pub kind: LineKind,
    pub file_index: usize,
    pub content: LineContent,
}

#[derive(Debug, Clone)]
pub enum LineContent {
    FileHeaderTop {
        path: String,
        stats: DiffStats,
    },
    FileHeaderBottom,
    HunkHeader {
        text: String,
    },
    UnifiedLine {
        old_num: Option<u32>,
        new_num: Option<u32>,
        prefix: char,
        segments: Vec<HighlightedSegment>, // Pre-computed syntax highlighting
    },
    SplitLine {
        old_num: Option<u32>,
        old_segments: Vec<HighlightedSegment>,
        new_num: Option<u32>,
        new_segments: Vec<HighlightedSegment>,
    },
    Empty,
}

impl LineContent {
    /// Get the new (right-side) line number if this is a code line.
    pub fn new_line_num(&self) -> Option<u32> {
        match self {
            LineContent::UnifiedLine { new_num, .. } => *new_num,
            LineContent::SplitLine { new_num, .. } => *new_num,
            _ => None,
        }
    }

    /// Get the old (left-side) line number if this is a code line.
    pub fn old_line_num(&self) -> Option<u32> {
        match self {
            LineContent::UnifiedLine { old_num, .. } => *old_num,
            LineContent::SplitLine { old_num, .. } => *old_num,
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    FileHeader,
    FileHeaderBottom,
    HunkHeader,
    Context,
    Addition,
    Deletion,
    Empty,
}

/// Compute word-level diff indices between two strings.
fn compute_changed_chars(old: &str, new: &str) -> (Vec<bool>, Vec<bool>) {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();

    let mut old_changed = vec![false; old_chars.len()];
    let mut new_changed = vec![false; new_chars.len()];

    // Simple word-based diff
    let old_words: Vec<&str> = old.split_inclusive(|c: char| {
        c.is_whitespace() || matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | ',' | ';' | ':' | '.' | '"' | '\'')
    }).collect();
    let new_words: Vec<&str> = new.split_inclusive(|c: char| {
        c.is_whitespace() || matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | ',' | ';' | ':' | '.' | '"' | '\'')
    }).collect();

    let lcs = compute_lcs(&old_words, &new_words);

    let mut old_pos = 0;
    let mut new_pos = 0;
    let mut old_char_pos = 0;
    let mut new_char_pos = 0;
    let mut lcs_idx = 0;

    while old_pos < old_words.len() || new_pos < new_words.len() {
        if lcs_idx < lcs.len() {
            let (lcs_old, lcs_new) = lcs[lcs_idx];

            while old_pos < lcs_old {
                let word_len = old_words[old_pos].chars().count();
                for i in old_char_pos..old_char_pos + word_len {
                    if i < old_changed.len() {
                        old_changed[i] = true;
                    }
                }
                old_char_pos += word_len;
                old_pos += 1;
            }

            while new_pos < lcs_new {
                let word_len = new_words[new_pos].chars().count();
                for i in new_char_pos..new_char_pos + word_len {
                    if i < new_changed.len() {
                        new_changed[i] = true;
                    }
                }
                new_char_pos += word_len;
                new_pos += 1;
            }

            if old_pos < old_words.len() && new_pos < new_words.len() {
                old_char_pos += old_words[old_pos].chars().count();
                new_char_pos += new_words[new_pos].chars().count();
                old_pos += 1;
                new_pos += 1;
            }
            lcs_idx += 1;
        } else {
            while old_pos < old_words.len() {
                let word_len = old_words[old_pos].chars().count();
                for i in old_char_pos..old_char_pos + word_len {
                    if i < old_changed.len() {
                        old_changed[i] = true;
                    }
                }
                old_char_pos += word_len;
                old_pos += 1;
            }
            while new_pos < new_words.len() {
                let word_len = new_words[new_pos].chars().count();
                for i in new_char_pos..new_char_pos + word_len {
                    if i < new_changed.len() {
                        new_changed[i] = true;
                    }
                }
                new_char_pos += word_len;
                new_pos += 1;
            }
        }
    }

    (old_changed, new_changed)
}

fn compute_lcs<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<(usize, usize)> {
    let m = old.len();
    let n = new.len();

    if m == 0 || n == 0 {
        return Vec::new();
    }

    let mut dp = vec![vec![0u16; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if old[i - 1] == new[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 && j > 0 {
        if old[i - 1] == new[j - 1] {
            result.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    result.reverse();
    result
}

/// Pre-compute syntax highlighting with word-diff overlay.
fn highlight_with_word_diff(content: &str, ext: &str, changed_chars: &[bool]) -> Vec<HighlightedSegment> {
    let syntax_spans = syntax::highlight_line(content, ext);
    let mut segments = Vec::new();
    let mut char_idx = 0;

    for span in syntax_spans {
        let text: String = span.content.to_string();
        let span_chars: Vec<char> = text.chars().collect();

        if span_chars.is_empty() {
            continue;
        }

        // Extract style info
        let fg = span.style.fg.unwrap_or(styles::FG_DEFAULT);
        let bold = span.style.add_modifier.contains(ratatui::style::Modifier::BOLD);
        let italic = span.style.add_modifier.contains(ratatui::style::Modifier::ITALIC);

        // Split span into changed/unchanged segments
        let mut seg_start = 0;
        while seg_start < span_chars.len() {
            let global_idx = char_idx + seg_start;
            let is_changed = global_idx < changed_chars.len() && changed_chars[global_idx];

            let mut seg_end = seg_start + 1;
            while seg_end < span_chars.len() {
                let next_global = char_idx + seg_end;
                let next_changed = next_global < changed_chars.len() && changed_chars[next_global];
                if next_changed != is_changed {
                    break;
                }
                seg_end += 1;
            }

            let segment_text: String = span_chars[seg_start..seg_end].iter().collect();
            segments.push(HighlightedSegment {
                text: segment_text,
                fg,
                bold,
                italic,
                is_changed,
            });

            seg_start = seg_end;
        }

        char_idx += span_chars.len();
    }

    segments
}

/// Pre-compute syntax highlighting without word-diff.
fn highlight_simple(content: &str, ext: &str) -> Vec<HighlightedSegment> {
    let syntax_spans = syntax::highlight_line(content, ext);

    syntax_spans
        .into_iter()
        .filter(|span| !span.content.is_empty())
        .map(|span| {
            let fg = span.style.fg.unwrap_or(styles::FG_DEFAULT);
            let bold = span.style.add_modifier.contains(ratatui::style::Modifier::BOLD);
            let italic = span.style.add_modifier.contains(ratatui::style::Modifier::ITALIC);
            HighlightedSegment {
                text: span.content.to_string(),
                fg,
                bold,
                italic,
                is_changed: false,
            }
        })
        .collect()
}

/// Build unified diff lines with pre-computed highlighting.
pub fn build_unified_lines(diff: &Diff, collapsed: &HashSet<usize>) -> Vec<DiffViewLine> {
    let mut lines = Vec::with_capacity(diff.files.iter().map(|f| f.hunks.iter().map(|h| h.lines.len()).sum::<usize>()).sum::<usize>() + diff.files.len() * 4);

    for (file_idx, file) in diff.files.iter().enumerate() {
        lines.push(DiffViewLine {
            kind: LineKind::FileHeader,
            file_index: file_idx,
            content: LineContent::FileHeaderTop {
                path: file.path.clone(),
                stats: file.stats,
            },
        });

        if collapsed.contains(&file_idx) {
            lines.push(DiffViewLine {
                kind: LineKind::FileHeaderBottom,
                file_index: file_idx,
                content: LineContent::FileHeaderBottom,
            });
            continue;
        }

        let ext = syntax::get_extension(&file.path);

        if file.is_binary {
            lines.push(DiffViewLine {
                kind: LineKind::Context,
                file_index: file_idx,
                content: LineContent::UnifiedLine {
                    old_num: None,
                    new_num: None,
                    prefix: ' ',
                    segments: vec![HighlightedSegment {
                        text: "Binary file".to_string(),
                        fg: styles::FG_MUTED,
                        bold: false,
                        italic: true,
                        is_changed: false,
                    }],
                },
            });
        } else {
            for hunk in &file.hunks {
                lines.push(DiffViewLine {
                    kind: LineKind::HunkHeader,
                    file_index: file_idx,
                    content: LineContent::HunkHeader {
                        text: hunk.header(),
                    },
                });

                let mut old_num = hunk.old_start;
                let mut new_num = hunk.new_start;

                let mut i = 0;
                while i < hunk.lines.len() {
                    match &hunk.lines[i] {
                        DiffLine::Context(c) => {
                            let segments = highlight_simple(c, ext);
                            lines.push(DiffViewLine {
                                kind: LineKind::Context,
                                file_index: file_idx,
                                content: LineContent::UnifiedLine {
                                    old_num: Some(old_num),
                                    new_num: Some(new_num),
                                    prefix: ' ',
                                    segments,
                                },
                            });
                            old_num += 1;
                            new_num += 1;
                            i += 1;
                        }
                        DiffLine::Deletion(del_content) => {
                            // Check for paired addition
                            if i + 1 < hunk.lines.len() {
                                if let DiffLine::Addition(add_content) = &hunk.lines[i + 1] {
                                    let (del_changed, add_changed) = compute_changed_chars(del_content, add_content);
                                    let del_segments = highlight_with_word_diff(del_content, ext, &del_changed);
                                    let add_segments = highlight_with_word_diff(add_content, ext, &add_changed);

                                    lines.push(DiffViewLine {
                                        kind: LineKind::Deletion,
                                        file_index: file_idx,
                                        content: LineContent::UnifiedLine {
                                            old_num: Some(old_num),
                                            new_num: None,
                                            prefix: '-',
                                            segments: del_segments,
                                        },
                                    });
                                    lines.push(DiffViewLine {
                                        kind: LineKind::Addition,
                                        file_index: file_idx,
                                        content: LineContent::UnifiedLine {
                                            old_num: None,
                                            new_num: Some(new_num),
                                            prefix: '+',
                                            segments: add_segments,
                                        },
                                    });
                                    old_num += 1;
                                    new_num += 1;
                                    i += 2;
                                    continue;
                                }
                            }
                            let segments = highlight_simple(del_content, ext);
                            lines.push(DiffViewLine {
                                kind: LineKind::Deletion,
                                file_index: file_idx,
                                content: LineContent::UnifiedLine {
                                    old_num: Some(old_num),
                                    new_num: None,
                                    prefix: '-',
                                    segments,
                                },
                            });
                            old_num += 1;
                            i += 1;
                        }
                        DiffLine::Addition(add_content) => {
                            let segments = highlight_simple(add_content, ext);
                            lines.push(DiffViewLine {
                                kind: LineKind::Addition,
                                file_index: file_idx,
                                content: LineContent::UnifiedLine {
                                    old_num: None,
                                    new_num: Some(new_num),
                                    prefix: '+',
                                    segments,
                                },
                            });
                            new_num += 1;
                            i += 1;
                        }
                    }
                }
            }
        }

        lines.push(DiffViewLine {
            kind: LineKind::FileHeaderBottom,
            file_index: file_idx,
            content: LineContent::FileHeaderBottom,
        });

        // Add spacing between file blocks
        lines.push(DiffViewLine {
            kind: LineKind::Empty,
            file_index: file_idx,
            content: LineContent::Empty,
        });
        lines.push(DiffViewLine {
            kind: LineKind::Empty,
            file_index: file_idx,
            content: LineContent::Empty,
        });
    }

    lines
}

/// Build split view lines with pre-computed highlighting.
pub fn build_split_lines(diff: &Diff, collapsed: &HashSet<usize>) -> Vec<DiffViewLine> {
    let mut lines = Vec::with_capacity(diff.files.iter().map(|f| f.hunks.iter().map(|h| h.lines.len()).sum::<usize>()).sum::<usize>() + diff.files.len() * 4);

    for (file_idx, file) in diff.files.iter().enumerate() {
        lines.push(DiffViewLine {
            kind: LineKind::FileHeader,
            file_index: file_idx,
            content: LineContent::FileHeaderTop {
                path: file.path.clone(),
                stats: file.stats,
            },
        });

        if collapsed.contains(&file_idx) {
            lines.push(DiffViewLine {
                kind: LineKind::FileHeaderBottom,
                file_index: file_idx,
                content: LineContent::FileHeaderBottom,
            });
            continue;
        }

        let ext = syntax::get_extension(&file.path);

        if file.is_binary {
            let seg = HighlightedSegment {
                text: "Binary file".to_string(),
                fg: styles::FG_MUTED,
                bold: false,
                italic: true,
                is_changed: false,
            };
            lines.push(DiffViewLine {
                kind: LineKind::Context,
                file_index: file_idx,
                content: LineContent::SplitLine {
                    old_num: None,
                    old_segments: vec![seg.clone()],
                    new_num: None,
                    new_segments: vec![seg],
                },
            });
        } else {
            for hunk in &file.hunks {
                lines.push(DiffViewLine {
                    kind: LineKind::HunkHeader,
                    file_index: file_idx,
                    content: LineContent::HunkHeader {
                        text: hunk.header(),
                    },
                });

                let mut old_num = hunk.old_start;
                let mut new_num = hunk.new_start;

                let mut i = 0;
                while i < hunk.lines.len() {
                    match &hunk.lines[i] {
                        DiffLine::Context(c) => {
                            let segments = highlight_simple(c, ext);
                            lines.push(DiffViewLine {
                                kind: LineKind::Context,
                                file_index: file_idx,
                                content: LineContent::SplitLine {
                                    old_num: Some(old_num),
                                    old_segments: segments.clone(),
                                    new_num: Some(new_num),
                                    new_segments: segments,
                                },
                            });
                            old_num += 1;
                            new_num += 1;
                            i += 1;
                        }
                        DiffLine::Deletion(del_content) => {
                            if i + 1 < hunk.lines.len() {
                                if let DiffLine::Addition(add_content) = &hunk.lines[i + 1] {
                                    let (del_changed, add_changed) = compute_changed_chars(del_content, add_content);
                                    let del_segments = highlight_with_word_diff(del_content, ext, &del_changed);
                                    let add_segments = highlight_with_word_diff(add_content, ext, &add_changed);

                                    lines.push(DiffViewLine {
                                        kind: LineKind::Context, // Side-by-side modification
                                        file_index: file_idx,
                                        content: LineContent::SplitLine {
                                            old_num: Some(old_num),
                                            old_segments: del_segments,
                                            new_num: Some(new_num),
                                            new_segments: add_segments,
                                        },
                                    });
                                    old_num += 1;
                                    new_num += 1;
                                    i += 2;
                                    continue;
                                }
                            }
                            let segments = highlight_simple(del_content, ext);
                            lines.push(DiffViewLine {
                                kind: LineKind::Deletion,
                                file_index: file_idx,
                                content: LineContent::SplitLine {
                                    old_num: Some(old_num),
                                    old_segments: segments,
                                    new_num: None,
                                    new_segments: Vec::new(),
                                },
                            });
                            old_num += 1;
                            i += 1;
                        }
                        DiffLine::Addition(add_content) => {
                            let segments = highlight_simple(add_content, ext);
                            lines.push(DiffViewLine {
                                kind: LineKind::Addition,
                                file_index: file_idx,
                                content: LineContent::SplitLine {
                                    old_num: None,
                                    old_segments: Vec::new(),
                                    new_num: Some(new_num),
                                    new_segments: segments,
                                },
                            });
                            new_num += 1;
                            i += 1;
                        }
                    }
                }
            }
        }

        lines.push(DiffViewLine {
            kind: LineKind::FileHeaderBottom,
            file_index: file_idx,
            content: LineContent::FileHeaderBottom,
        });

        // Add spacing between file blocks
        lines.push(DiffViewLine {
            kind: LineKind::Empty,
            file_index: file_idx,
            content: LineContent::Empty,
        });
        lines.push(DiffViewLine {
            kind: LineKind::Empty,
            file_index: file_idx,
            content: LineContent::Empty,
        });
    }

    lines
}

/// Find line index where a file starts.
pub fn find_file_start(lines: &[DiffViewLine], file_index: usize) -> usize {
    lines
        .iter()
        .position(|l| l.file_index == file_index && l.kind == LineKind::FileHeader)
        .unwrap_or(0)
}

/// Apply visual selection highlight to a line.
/// Uses brighter backgrounds that are visible on diff lines.
fn apply_visual_selection_highlight(mut line: Line<'static>) -> Line<'static> {
    for span in line.spans.iter_mut() {
        // Map current background to brighter selection variant
        let new_bg = match span.style.bg {
            Some(styles::BG_ADDITION_LINE) => styles::BG_ADDITION_SELECTED,
            Some(styles::BG_DELETION_LINE) => styles::BG_DELETION_SELECTED,
            Some(styles::BG_DEFAULT) | None => styles::BG_CONTEXT_SELECTED,
            Some(other) => other, // Keep special backgrounds (word highlights, etc.)
        };
        span.style = span.style.bg(new_bg);
    }
    line
}

/// Render a GitHub-style inline comment box.
/// Returns multiple lines for the comment display.
fn render_comment_box(comment: &Comment, width: u16, focused: bool) -> Vec<Line<'static>> {
    let w = width as usize;
    let inner_w = w.saturating_sub(6); // Account for borders and padding

    // Use accent color when focused
    let border_color = if focused {
        styles::FG_HUNK // Accent color when selected
    } else if comment.resolved {
        styles::FG_MUTED
    } else {
        styles::FG_BORDER
    };
    let bg_color = styles::BG_SIDEBAR;

    let mut lines = Vec::new();

    // ── Header line: "┌─ Comment on lines L69 to L75 ─────────────────┐"
    let line_range = comment.line_range_display();
    let header_text = format!(" Comment on lines {} ", line_range);
    let header_fill_len = inner_w.saturating_sub(header_text.len() + 2);
    let header_fill = "─".repeat(header_fill_len);

    lines.push(Line::from(vec![
        Span::styled("  ┌─", Style::default().fg(border_color)),
        Span::styled(header_text, Style::default().fg(styles::FG_HUNK)),
        Span::styled(header_fill, Style::default().fg(border_color)),
        Span::styled("┐", Style::default().fg(border_color)),
    ]));

    // ── Author line: "│ synoet • 2h ago"
    let resolved_badge = if comment.resolved { " ✓ Resolved" } else { "" };
    let author_line = format!(" {} • {}{}", comment.author, comment.relative_time(), resolved_badge);
    let author_pad = inner_w.saturating_sub(author_line.chars().count());

    let author_style = if comment.resolved {
        Style::default().fg(styles::FG_MUTED).bg(bg_color)
    } else {
        Style::default().fg(styles::FG_DEFAULT).bg(bg_color)
    };

    lines.push(Line::from(vec![
        Span::styled("  │", Style::default().fg(border_color)),
        Span::styled(author_line, author_style),
        Span::styled(" ".repeat(author_pad), Style::default().bg(bg_color)),
        Span::styled("│", Style::default().fg(border_color)),
    ]));

    // ── Empty line
    let empty_fill = " ".repeat(inner_w);
    lines.push(Line::from(vec![
        Span::styled("  │", Style::default().fg(border_color)),
        Span::styled(empty_fill.clone(), Style::default().bg(bg_color)),
        Span::styled("│", Style::default().fg(border_color)),
    ]));

    // ── Comment body (may wrap to multiple lines)
    let body_style = if comment.resolved {
        Style::default().fg(styles::FG_MUTED).bg(bg_color).add_modifier(Modifier::ITALIC)
    } else {
        Style::default().fg(styles::FG_DEFAULT).bg(bg_color)
    };

    // Simple word wrap for comment body
    for body_line in wrap_text(&comment.body, inner_w.saturating_sub(2)) {
        let line_text = format!(" {}", body_line);
        let pad_len = inner_w.saturating_sub(line_text.chars().count());
        lines.push(Line::from(vec![
            Span::styled("  │", Style::default().fg(border_color)),
            Span::styled(line_text, body_style),
            Span::styled(" ".repeat(pad_len), Style::default().bg(bg_color)),
            Span::styled("│", Style::default().fg(border_color)),
        ]));
    }

    // ── Replies
    for reply in &comment.replies {
        // Empty separator
        lines.push(Line::from(vec![
            Span::styled("  │", Style::default().fg(border_color)),
            Span::styled(empty_fill.clone(), Style::default().bg(bg_color)),
            Span::styled("│", Style::default().fg(border_color)),
        ]));

        // Reply header
        let reply_header = format!(" ↳ {} • {}", reply.author, reply.relative_time());
        let reply_pad = inner_w.saturating_sub(reply_header.chars().count());
        lines.push(Line::from(vec![
            Span::styled("  │", Style::default().fg(border_color)),
            Span::styled(reply_header, Style::default().fg(styles::FG_MUTED).bg(bg_color)),
            Span::styled(" ".repeat(reply_pad), Style::default().bg(bg_color)),
            Span::styled("│", Style::default().fg(border_color)),
        ]));

        // Reply body
        for reply_line in wrap_text(&reply.body, inner_w.saturating_sub(4)) {
            let line_text = format!("   {}", reply_line);
            let pad_len = inner_w.saturating_sub(line_text.chars().count());
            lines.push(Line::from(vec![
                Span::styled("  │", Style::default().fg(border_color)),
                Span::styled(line_text, Style::default().fg(styles::FG_DEFAULT).bg(bg_color)),
                Span::styled(" ".repeat(pad_len), Style::default().bg(bg_color)),
                Span::styled("│", Style::default().fg(border_color)),
            ]));
        }
    }

    // ── Footer with hints
    lines.push(Line::from(vec![
        Span::styled("  │", Style::default().fg(border_color)),
        Span::styled(empty_fill.clone(), Style::default().bg(bg_color)),
        Span::styled("│", Style::default().fg(border_color)),
    ]));

    let hints = if comment.resolved {
        " R unresolve │ r reply │ D delete"
    } else {
        " R resolve │ r reply │ D delete"
    };
    let hints_pad = inner_w.saturating_sub(hints.len());
    lines.push(Line::from(vec![
        Span::styled("  │", Style::default().fg(border_color)),
        Span::styled(hints, Style::default().fg(styles::FG_MUTED).bg(bg_color)),
        Span::styled(" ".repeat(hints_pad), Style::default().bg(bg_color)),
        Span::styled("│", Style::default().fg(border_color)),
    ]));

    // ── Bottom border
    let bottom_fill = "─".repeat(inner_w);
    lines.push(Line::from(vec![
        Span::styled("  └", Style::default().fg(border_color)),
        Span::styled(bottom_fill, Style::default().fg(border_color)),
        Span::styled("┘", Style::default().fg(border_color)),
    ]));

    lines
}

/// Render a draft comment box (for comment input mode).
fn render_draft_comment_box(
    file_path: &str,
    start_line: usize,
    end_line: usize,
    body: &str,
    width: u16,
) -> Vec<Line<'static>> {
    let w = width as usize;
    let inner_w = w.saturating_sub(6);

    let border_color = styles::FG_HUNK; // Accent color for active draft
    let bg_color = styles::BG_SIDEBAR;

    let mut lines = Vec::new();

    // Header
    let line_range = if start_line == end_line {
        format!("L{}", start_line + 1)
    } else {
        format!("L{}-L{}", start_line + 1, end_line + 1)
    };
    let header_text = format!(" New comment on {} ", line_range);
    let header_fill_len = inner_w.saturating_sub(header_text.len() + 2);
    let header_fill = "─".repeat(header_fill_len);

    lines.push(Line::from(vec![
        Span::styled("  ┌─", Style::default().fg(border_color)),
        Span::styled(header_text, Style::default().fg(styles::FG_HUNK)),
        Span::styled(header_fill, Style::default().fg(border_color)),
        Span::styled("┐", Style::default().fg(border_color)),
    ]));

    // File path (truncated)
    let _file_display: String = file_path.chars().rev().take(inner_w.saturating_sub(2)).collect::<String>().chars().rev().collect();

    // Empty line
    let empty_fill = " ".repeat(inner_w);
    lines.push(Line::from(vec![
        Span::styled("  │", Style::default().fg(border_color)),
        Span::styled(empty_fill.clone(), Style::default().bg(bg_color)),
        Span::styled("│", Style::default().fg(border_color)),
    ]));

    // Body input area with cursor at end of text
    let text_style = Style::default().fg(styles::FG_DEFAULT).bg(bg_color);
    let placeholder_style = Style::default().fg(styles::FG_MUTED).bg(bg_color);
    let max_content_width = inner_w.saturating_sub(2); // -2 for leading space and cursor

    if body.is_empty() {
        // Show placeholder with cursor at start
        let placeholder = "Type your comment...";
        let pad_len = inner_w.saturating_sub(1 + 1 + placeholder.chars().count()); // space + cursor + text

        lines.push(Line::from(vec![
            Span::styled("  │", Style::default().fg(border_color)),
            Span::styled(" ", Style::default().bg(bg_color)),
            Span::styled("█", Style::default().fg(styles::FG_HUNK).bg(bg_color)),
            Span::styled(placeholder, placeholder_style),
            Span::styled(" ".repeat(pad_len), Style::default().bg(bg_color)),
            Span::styled("│", Style::default().fg(border_color)),
        ]));
    } else {
        // Split body into lines that fit, preserving all characters including trailing spaces
        let body_chars: Vec<char> = body.chars().collect();
        let mut start = 0;

        while start < body_chars.len() {
            let end = (start + max_content_width).min(body_chars.len());
            let line_str: String = body_chars[start..end].iter().collect();
            let is_last = end >= body_chars.len();

            if is_last {
                // Last line with cursor at end
                let line_text = format!(" {}", line_str);
                let content_len = line_text.chars().count() + 1; // +1 for cursor
                let pad_len = inner_w.saturating_sub(content_len);

                lines.push(Line::from(vec![
                    Span::styled("  │", Style::default().fg(border_color)),
                    Span::styled(line_text, text_style),
                    Span::styled("█", Style::default().fg(styles::FG_HUNK).bg(bg_color)),
                    Span::styled(" ".repeat(pad_len), Style::default().bg(bg_color)),
                    Span::styled("│", Style::default().fg(border_color)),
                ]));
            } else {
                // Not the last line
                let line_text = format!(" {}", line_str);
                let pad_len = inner_w.saturating_sub(line_text.chars().count());

                lines.push(Line::from(vec![
                    Span::styled("  │", Style::default().fg(border_color)),
                    Span::styled(line_text, text_style),
                    Span::styled(" ".repeat(pad_len), Style::default().bg(bg_color)),
                    Span::styled("│", Style::default().fg(border_color)),
                ]));
            }

            start = end;
        }
    }

    // Empty line
    lines.push(Line::from(vec![
        Span::styled("  │", Style::default().fg(border_color)),
        Span::styled(empty_fill.clone(), Style::default().bg(bg_color)),
        Span::styled("│", Style::default().fg(border_color)),
    ]));

    // Hints
    let hints = " Enter submit │ Esc cancel";
    let hints_pad = inner_w.saturating_sub(hints.len());
    lines.push(Line::from(vec![
        Span::styled("  │", Style::default().fg(border_color)),
        Span::styled(hints, Style::default().fg(styles::FG_MUTED).bg(bg_color)),
        Span::styled(" ".repeat(hints_pad), Style::default().bg(bg_color)),
        Span::styled("│", Style::default().fg(border_color)),
    ]));

    // Bottom border
    let bottom_fill = "─".repeat(inner_w);
    lines.push(Line::from(vec![
        Span::styled("  └", Style::default().fg(border_color)),
        Span::styled(bottom_fill, Style::default().fg(border_color)),
        Span::styled("┘", Style::default().fg(border_color)),
    ]));

    lines
}

/// Simple word wrapping for text.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Render unified diff view.
#[allow(clippy::too_many_arguments)]
pub fn render_unified(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    lines: &[DiffViewLine],
    scroll: usize,
    cursor: usize,
    current_file: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    stale_viewed: &HashSet<usize>,
    diff_source: DiffSource,
    uncommitted_files: &HashSet<String>,
    comments: &[Comment],
    show_comments: bool,
    visual_selection: Option<(usize, usize)>,
    focused_comment: Option<i64>,
    draft_comment: Option<&(String, usize, usize, String)>,
) {
    // Gutter width: always 2 chars for consistent layout
    let gutter_width = 2u16;
    let content_width = area.width.saturating_sub(gutter_width);

    // Find the sticky header for current scroll position
    let sticky_header = find_sticky_header(lines, scroll, current_file);

    // Reserve space for sticky header if needed
    let (sticky_area, content_area) = if sticky_header.is_some() {
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    // Render sticky header if present
    if let (Some(sticky_rect), Some((path, stats, file_idx))) = (sticky_area, sticky_header) {
        let is_viewed = viewed.contains(&file_idx);
        let is_stale = stale_viewed.contains(&file_idx);
        // Use content_width (same as regular file headers) to ensure alignment
        let mut sticky_line = render_sticky_header(&path, &stats, file_idx, current_file, collapsed, is_viewed, is_stale, content_width);

        // Build full line: gutter (2 chars) + header content (same as regular lines)
        let mut spans = vec![Span::styled("  ", Style::default())];
        spans.extend(sticky_line.spans.drain(..));

        let sticky_para = Paragraph::new(vec![Line::from(spans)]);
        frame.render_widget(sticky_para, sticky_rect);
    }

    let visible_height = content_area.height as usize;

    // Build visible lines with visual selection highlighting and inline comments
    let mut visible_lines: Vec<Line> = Vec::with_capacity(visible_height);
    let mut line_idx = scroll;
    let mut rendered_count = 0;

    while rendered_count < visible_height && line_idx < lines.len() {
        let line = &lines[line_idx];
        let absolute_line_idx = line_idx;

        // Check if this line is the cursor line
        let is_cursor_line = absolute_line_idx == cursor;

        // Check if this line is part of visual selection
        let is_selected = visual_selection
            .map(|(start, end)| absolute_line_idx >= start && absolute_line_idx <= end)
            .unwrap_or(false);

        let mut rendered = render_unified_line(line, current_file, collapsed, viewed, stale_viewed, content_width);

        // Apply visual selection highlighting
        if is_selected {
            rendered = apply_visual_selection_highlight(rendered);
        }

        // Determine if this line's file has uncommitted changes
        let file_path = diff.files.get(line.file_index).map(|f| &f.path);
        let is_uncommitted = file_path
            .map(|p| uncommitted_files.contains(p))
            .unwrap_or(false);

        // Check if this line is within a comment range (for visual indication)
        let is_in_comment_range = if show_comments {
            let source_line = line.content.new_line_num().map(|n| n as usize);
            let path = diff.files.get(line.file_index).map(|f| f.path.as_str());
            match (path, source_line) {
                (Some(p), Some(ln)) => comments.iter().any(|c| {
                    c.file_path == p && ln >= c.start_line && ln <= c.end_line
                }),
                _ => false,
            }
        } else {
            false
        };

        // Prepend gutter (always present for consistent layout)
        // Priority: cursor > uncommitted > visual selection > comment range > default
        let gutter_style = if is_cursor_line {
            Style::default().fg(styles::FG_CURSOR)
        } else if diff_source == DiffSource::All && is_uncommitted {
            Style::default().fg(styles::FG_WARNING)
        } else if is_selected {
            Style::default().fg(styles::FG_HUNK)
        } else if is_in_comment_range {
            Style::default().fg(styles::FG_HUNK)
        } else {
            Style::default().fg(styles::FG_BORDER)
        };

        let gutter_char = if is_cursor_line {
            "▶ " // Cursor line indicator (arrow)
        } else if diff_source == DiffSource::All && is_uncommitted {
            "▎ " // Orange bar for uncommitted
        } else if is_selected {
            "▌ " // Visual selection indicator
        } else if is_in_comment_range {
            "┃ " // Comment range indicator (blue bar)
        } else {
            "  " // Empty gutter
        };

        let mut spans = vec![Span::styled(gutter_char, gutter_style)];
        spans.extend(rendered.spans.drain(..));
        visible_lines.push(Line::from(spans));
        rendered_count += 1;

        // Render inline comments for this line if enabled
        if show_comments {
            let file_path = diff.files.get(line.file_index).map(|f| f.path.as_str());
            let source_line_num = line.content.new_line_num().map(|n| n as usize);
            if let (Some(path), Some(line_num)) = (file_path, source_line_num) {
                for comment in comments.iter().filter(|c| {
                    c.file_path == path && line_num >= c.start_line && line_num <= c.end_line
                }) {
                    // Only render comment after the last line of its range
                    if line_num == comment.end_line {
                        let is_focused = focused_comment == Some(comment.id);
                        let comment_lines = render_comment_box(comment, content_width + gutter_width, is_focused);
                        for comment_line in comment_lines {
                            if rendered_count >= visible_height {
                                break;
                            }
                            visible_lines.push(comment_line);
                            rendered_count += 1;
                        }
                    }
                }

                // Render draft comment if this is the end line of the draft
                if let Some((draft_path, draft_start, draft_end, draft_body)) = draft_comment {
                    if draft_path == path && absolute_line_idx == *draft_end {
                        let draft_lines = render_draft_comment_box(
                            draft_path,
                            *draft_start,
                            *draft_end,
                            draft_body,
                            content_width + gutter_width,
                        );
                        for draft_line in draft_lines {
                            if rendered_count >= visible_height {
                                break;
                            }
                            visible_lines.push(draft_line);
                            rendered_count += 1;
                        }
                    }
                }
            }
        }

        line_idx += 1;
    }

    let content = Paragraph::new(visible_lines)
        .style(Style::default());
    frame.render_widget(content, content_area);

    if lines.len() > visible_height {
        render_scrollbar(frame, content_area, lines.len(), scroll);
    }
}

/// Render split diff view.
#[allow(clippy::too_many_arguments)]
pub fn render_split(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    lines: &[DiffViewLine],
    scroll: usize,
    cursor: usize,
    current_file: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    stale_viewed: &HashSet<usize>,
    diff_source: DiffSource,
    uncommitted_files: &HashSet<String>,
    comments: &[Comment],
    show_comments: bool,
    visual_selection: Option<(usize, usize)>,
    focused_comment: Option<i64>,
    draft_comment: Option<&(String, usize, usize, String)>,
) {
    // Gutter width: always 2 chars for consistent layout
    let gutter_width = 2u16;

    // Find the sticky header for current scroll position
    let sticky_header = find_sticky_header(lines, scroll, current_file);

    // Reserve space for sticky header if needed
    let (sticky_area, content_area) = if sticky_header.is_some() {
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    // Render sticky header if present
    if let (Some(area), Some((path, stats, file_idx))) = (sticky_area, sticky_header) {
        let is_viewed = viewed.contains(&file_idx);
        let is_stale = stale_viewed.contains(&file_idx);
        let sticky_line = render_sticky_header(&path, &stats, file_idx, current_file, collapsed, is_viewed, is_stale, area.width);
        let sticky_para = Paragraph::new(vec![sticky_line]);
        frame.render_widget(sticky_para, area);
    }

    let half_width = content_area.width.saturating_sub(gutter_width) / 2;
    let visible_height = content_area.height as usize;

    // Build visible lines with visual selection highlighting and inline comments
    let mut visible_lines: Vec<Line> = Vec::with_capacity(visible_height);
    let mut line_idx = scroll;
    let mut rendered_count = 0;

    while rendered_count < visible_height && line_idx < lines.len() {
        let line = &lines[line_idx];
        let absolute_line_idx = line_idx;

        // Check if this line is the cursor line
        let is_cursor_line = absolute_line_idx == cursor;

        // Check if this line is part of visual selection
        let is_selected = visual_selection
            .map(|(start, end)| absolute_line_idx >= start && absolute_line_idx <= end)
            .unwrap_or(false);

        let mut rendered = render_split_line(line, current_file, collapsed, viewed, stale_viewed, half_width as usize, content_area.width.saturating_sub(gutter_width));

        // Apply visual selection highlighting
        if is_selected {
            rendered = apply_visual_selection_highlight(rendered);
        }

        // Determine if this line's file has uncommitted changes
        let file_path = diff.files.get(line.file_index).map(|f| &f.path);
        let is_uncommitted = file_path
            .map(|p| uncommitted_files.contains(p))
            .unwrap_or(false);

        // Check if this line is within a comment range (for visual indication)
        let is_in_comment_range = if show_comments {
            let source_line = line.content.new_line_num().map(|n| n as usize);
            let path = diff.files.get(line.file_index).map(|f| f.path.as_str());
            match (path, source_line) {
                (Some(p), Some(ln)) => comments.iter().any(|c| {
                    c.file_path == p && ln >= c.start_line && ln <= c.end_line
                }),
                _ => false,
            }
        } else {
            false
        };

        // Prepend gutter (always present for consistent layout)
        // Priority: cursor > uncommitted > visual selection > comment range > default
        let gutter_style = if is_cursor_line {
            Style::default().fg(styles::FG_CURSOR)
        } else if diff_source == DiffSource::All && is_uncommitted {
            Style::default().fg(styles::FG_WARNING)
        } else if is_selected {
            Style::default().fg(styles::FG_HUNK)
        } else if is_in_comment_range {
            Style::default().fg(styles::FG_HUNK)
        } else {
            Style::default().fg(styles::FG_BORDER)
        };

        let gutter_char = if is_cursor_line {
            "▶ " // Cursor line indicator (arrow)
        } else if diff_source == DiffSource::All && is_uncommitted {
            "▎ " // Orange bar for uncommitted
        } else if is_selected {
            "▌ " // Visual selection indicator
        } else if is_in_comment_range {
            "┃ " // Comment range indicator (blue bar)
        } else {
            "  " // Empty gutter
        };

        let mut spans = vec![Span::styled(gutter_char, gutter_style)];
        spans.extend(rendered.spans.drain(..));
        visible_lines.push(Line::from(spans));
        rendered_count += 1;

        // Render inline comments for this line if enabled
        if show_comments {
            let file_path = diff.files.get(line.file_index).map(|f| f.path.as_str());
            let source_line_num = line.content.new_line_num().map(|n| n as usize);
            if let (Some(path), Some(line_num)) = (file_path, source_line_num) {
                for comment in comments.iter().filter(|c| {
                    c.file_path == path && line_num >= c.start_line && line_num <= c.end_line
                }) {
                    // Only render comment after the last line of its range
                    if line_num == comment.end_line {
                        let is_focused = focused_comment == Some(comment.id);
                        let comment_lines = render_comment_box(comment, content_area.width, is_focused);
                        for comment_line in comment_lines {
                            if rendered_count >= visible_height {
                                break;
                            }
                            visible_lines.push(comment_line);
                            rendered_count += 1;
                        }
                    }
                }

                // Render draft comment if this is the end line of the draft
                if let Some((draft_path, draft_start, draft_end, draft_body)) = draft_comment {
                    if draft_path == path && absolute_line_idx == *draft_end {
                        let draft_lines = render_draft_comment_box(
                            draft_path,
                            *draft_start,
                            *draft_end,
                            draft_body,
                            content_area.width,
                        );
                        for draft_line in draft_lines {
                            if rendered_count >= visible_height {
                                break;
                            }
                            visible_lines.push(draft_line);
                            rendered_count += 1;
                        }
                    }
                }
            }
        }

        line_idx += 1;
    }

    let content = Paragraph::new(visible_lines)
        .style(Style::default());
    frame.render_widget(content, content_area);

    if lines.len() > visible_height {
        render_scrollbar(frame, content_area, lines.len(), scroll);
    }
}

/// Find the file header that should be sticky at the current scroll position.
fn find_sticky_header(lines: &[DiffViewLine], scroll: usize, _current_file: usize) -> Option<(String, DiffStats, usize)> {
    // Look backwards from scroll position to find the most recent file header
    if scroll == 0 {
        return None;
    }

    // Check if the current scroll position is past a file header
    for i in (0..scroll).rev() {
        if let Some(line) = lines.get(i) {
            if let LineContent::FileHeaderTop { path, stats } = &line.content {
                // Only show sticky header if we're past the header line
                return Some((path.clone(), *stats, line.file_index));
            }
            // If we hit a file bottom, the header is still visible
            if matches!(line.content, LineContent::FileHeaderBottom) && line.file_index != lines.get(scroll).map(|l| l.file_index).unwrap_or(0) {
                continue;
            }
        }
    }
    None
}

/// Render a sticky file header (matches render_file_header_top style).
fn render_sticky_header(
    path: &str,
    stats: &DiffStats,
    file_index: usize,
    current_file: usize,
    collapsed: &HashSet<usize>,
    is_viewed: bool,
    is_stale: bool,
    width: u16,
) -> Line<'static> {
    let w = width as usize;
    let is_collapsed = collapsed.contains(&file_index);
    let is_current = file_index == current_file;

    let toggle = if is_collapsed { "›" } else { "⌄" };
    let viewed_icon = if is_viewed { " ✓" } else { "" };
    let stale_icon = if is_stale { " ●" } else { "" };
    let border_color = if is_current { styles::FG_HUNK } else { styles::FG_BORDER };
    let path_color = if is_current { styles::FG_DEFAULT } else { styles::FG_PATH };

    let add_str = format!("+{}", stats.additions);
    let del_str = format!("-{}", stats.deletions);

    // Calculate exact widths for alignment
    // Left: space + toggle + space + path
    let left_len = 1 + toggle.chars().count() + 1 + path.chars().count();
    // Right: +N + 2 spaces + -M + viewed + stale + trailing space
    let right_len = add_str.chars().count() + 2 + del_str.chars().count() + viewed_icon.chars().count() + stale_icon.chars().count() + 1;

    // Content lines are w-1 wide, so header should also be w-1
    let inner_width = w.saturating_sub(3);
    let padding_len = inner_width.saturating_sub(left_len + right_len);

    Line::from(vec![
        Span::styled("╭", Style::default().fg(border_color)),
        Span::styled(
            format!(" {} {}", toggle, path),
            Style::default().fg(path_color).add_modifier(if is_current { Modifier::BOLD } else { Modifier::empty() }),
        ),
        Span::styled(" ".repeat(padding_len), Style::default()),
        Span::styled(add_str, Style::default().fg(styles::FG_ADDITION)),
        Span::styled("  ", Style::default()),
        Span::styled(del_str, Style::default().fg(styles::FG_DELETION)),
        Span::styled(viewed_icon, Style::default().fg(styles::FG_ADDITION)),
        Span::styled(stale_icon, Style::default().fg(styles::FG_WARNING)),
        Span::styled(" ", Style::default()),
        Span::styled("╮", Style::default().fg(border_color)),
    ])
}

#[inline]
fn render_unified_line(
    line: &DiffViewLine,
    current_file: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    stale_viewed: &HashSet<usize>,
    width: u16,
) -> Line<'static> {
    let w = width as usize;
    let is_current_file = line.file_index == current_file;
    let border_style = if is_current_file { styles::style_border_selected() } else { styles::style_border() };

    match &line.content {
        LineContent::FileHeaderTop { path, stats } => {
            let is_viewed = viewed.contains(&line.file_index);
            let is_stale = stale_viewed.contains(&line.file_index);
            render_file_header_top(path, stats, line.file_index, current_file, collapsed, is_viewed, is_stale, width)
        }
        LineContent::FileHeaderBottom => {
            render_file_header_bottom(line.file_index, current_file, width)
        }
        LineContent::HunkHeader { text } => {
            // Content lines are w-1, so hunk header should be too
            let inner_width = w.saturating_sub(3);
            let expand_area = "  ⋯  ";
            let hunk_text = format!(" {} ", text);
            let used_width = expand_area.chars().count() + hunk_text.chars().count();
            let padding_len = inner_width.saturating_sub(used_width);

            Line::from(vec![
                Span::styled(styles::BORDER_VERTICAL, border_style),
                Span::styled(expand_area, Style::default().fg(styles::FG_HUNK).bg(styles::BG_HUNK_EXPAND)),
                Span::styled(hunk_text, styles::style_hunk_header()),
                Span::styled(" ".repeat(padding_len), Style::default().bg(styles::BG_HUNK_HEADER)),
                Span::styled(styles::BORDER_VERTICAL, border_style),
            ])
        }
        LineContent::UnifiedLine {
            old_num,
            new_num,
            prefix,
            segments,
        } => {
            let old_str = old_num.map(|n| format!("{:>4}", n)).unwrap_or_else(|| "    ".into());
            let new_str = new_num.map(|n| format!("{:>4}", n)).unwrap_or_else(|| "    ".into());

            let (margin_bg, line_bg, word_bg, prefix_style) = match line.kind {
                LineKind::Addition => (Some(styles::BG_ADDITION_MARGIN), Some(styles::BG_ADDITION_LINE), Some(styles::BG_ADDITION_WORD), styles::style_addition()),
                LineKind::Deletion => (Some(styles::BG_DELETION_MARGIN), Some(styles::BG_DELETION_LINE), Some(styles::BG_DELETION_WORD), styles::style_deletion()),
                _ => (None, None, None, styles::style_context()), // No background for context lines
            };

            let content_width = w.saturating_sub(14);

            let mut spans = Vec::with_capacity(segments.len() + 6);
            spans.push(Span::styled(styles::BORDER_VERTICAL, border_style));

            // Line numbers with optional background
            let line_num_style = if let Some(bg) = margin_bg {
                Style::default().fg(styles::FG_LINE_NUM).bg(bg)
            } else {
                Style::default().fg(styles::FG_LINE_NUM)
            };
            spans.push(Span::styled(old_str, line_num_style));
            spans.push(Span::styled(" ", line_num_style));
            spans.push(Span::styled(new_str, line_num_style));

            // Prefix with optional background
            let prefix_with_bg = if let Some(bg) = line_bg {
                prefix_style.bg(bg)
            } else {
                prefix_style
            };
            spans.push(Span::styled(format!(" {}", prefix), prefix_with_bg));

            // Render cached segments with appropriate backgrounds
            let mut char_count = 0;
            for seg in segments {
                if char_count >= content_width {
                    break;
                }
                let bg = if seg.is_changed { word_bg } else { line_bg };
                let mut style = Style::default().fg(seg.fg);
                if let Some(bg_color) = bg {
                    style = style.bg(bg_color);
                }
                if seg.bold {
                    style = style.add_modifier(ratatui::style::Modifier::BOLD);
                }
                if seg.italic {
                    style = style.add_modifier(ratatui::style::Modifier::ITALIC);
                }
                let seg_chars = seg.text.chars().count();
                if char_count + seg_chars > content_width {
                    let take = content_width - char_count;
                    let truncated: String = seg.text.chars().take(take).collect();
                    spans.push(Span::styled(truncated, style));
                    char_count = content_width;
                } else {
                    spans.push(Span::styled(seg.text.clone(), style));
                    char_count += seg_chars;
                }
            }

            // Pad remaining (no background for context)
            if char_count < content_width {
                let pad_style = if let Some(bg) = line_bg {
                    Style::default().bg(bg)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(" ".repeat(content_width - char_count), pad_style));
            }

            spans.push(Span::styled(styles::BORDER_VERTICAL, border_style));
            Line::from(spans)
        }
        LineContent::Empty => {
            Line::from(vec![Span::raw(" ".repeat(w))]) // No background
        }
        _ => Line::from(""),
    }
}

fn render_file_header_top(
    path: &str,
    stats: &DiffStats,
    file_index: usize,
    current_file: usize,
    collapsed: &HashSet<usize>,
    is_viewed: bool,
    is_stale: bool,
    width: u16,
) -> Line<'static> {
    let w = width as usize;
    let is_collapsed = collapsed.contains(&file_index);
    let is_current = file_index == current_file;

    let toggle = if is_collapsed { "›" } else { "⌄" };
    let border_color = if is_current { styles::FG_HUNK } else { styles::FG_BORDER };
    let path_color = if is_current { styles::FG_DEFAULT } else { styles::FG_PATH };

    // Stats on right
    let add_str = format!("+{}", stats.additions);
    let del_str = format!("-{}", stats.deletions);
    let viewed_icon = if is_viewed { " ✓" } else { "" };
    let stale_icon = if is_stale { " ●" } else { "" };

    // Calculate exact widths for alignment
    // Left: space + toggle + space + path
    let left_len = 1 + toggle.chars().count() + 1 + path.chars().count();
    // Right: +N + 2 spaces + -M + viewed + stale + trailing space
    let right_len = add_str.chars().count() + 2 + del_str.chars().count() + viewed_icon.chars().count() + stale_icon.chars().count() + 1;

    // Content lines are w-1 wide, so header should also be w-1
    let inner_width = w.saturating_sub(3);
    let padding_len = inner_width.saturating_sub(left_len + right_len);

    Line::from(vec![
        Span::styled("╭", Style::default().fg(border_color)),
        Span::styled(
            format!(" {} {}", toggle, path),
            Style::default().fg(path_color).add_modifier(if is_current { Modifier::BOLD } else { Modifier::empty() }),
        ),
        Span::styled(" ".repeat(padding_len), Style::default()),
        Span::styled(add_str, Style::default().fg(styles::FG_ADDITION)),
        Span::styled("  ", Style::default()),
        Span::styled(del_str, Style::default().fg(styles::FG_DELETION)),
        Span::styled(viewed_icon, Style::default().fg(styles::FG_ADDITION)),
        Span::styled(stale_icon, Style::default().fg(styles::FG_WARNING)),
        Span::styled(" ", Style::default()),
        Span::styled("╮", Style::default().fg(border_color)),
    ])
}

fn render_file_header_bottom(file_index: usize, current_file: usize, width: u16) -> Line<'static> {
    let w = width as usize;
    let is_current = file_index == current_file;
    let border_color = if is_current { styles::FG_HUNK } else { styles::FG_BORDER };

    // Content lines are: │ + 4 + 1 + 4 + 2 + content + │ = 12 + content + 1 = 13 + (w-14) = w-1
    // So bottom border should also be w-1: ╰ + (w-3) + ╯ = 1 + (w-3) + 1 = w-1
    let inner_width = w.saturating_sub(3);

    Line::from(vec![
        Span::styled("╰", Style::default().fg(border_color)),
        Span::styled("─".repeat(inner_width), Style::default().fg(border_color)),
        Span::styled("╯", Style::default().fg(border_color)),
    ])
}

#[inline]
fn render_split_line(
    line: &DiffViewLine,
    current_file: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    stale_viewed: &HashSet<usize>,
    half_width: usize,
    full_width: u16,
) -> Line<'static> {
    let w = full_width as usize;
    let is_current_file = line.file_index == current_file;
    let border_style = if is_current_file { styles::style_border_selected() } else { styles::style_border() };

    match &line.content {
        LineContent::FileHeaderTop { path, stats } => {
            let is_viewed = viewed.contains(&line.file_index);
            let is_stale = stale_viewed.contains(&line.file_index);
            render_file_header_top(path, stats, line.file_index, current_file, collapsed, is_viewed, is_stale, full_width)
        }
        LineContent::FileHeaderBottom => {
            render_file_header_bottom(line.file_index, current_file, full_width)
        }
        LineContent::HunkHeader { text } => {
            // Content lines are w-1, so hunk header should be too
            let inner_width = w.saturating_sub(3);
            let expand_area = "  ⋯  ";
            let hunk_text = format!(" {} ", text);
            let used_width = expand_area.chars().count() + hunk_text.chars().count();
            let padding_len = inner_width.saturating_sub(used_width);

            Line::from(vec![
                Span::styled(styles::BORDER_VERTICAL, border_style),
                Span::styled(expand_area, Style::default().fg(styles::FG_HUNK).bg(styles::BG_HUNK_EXPAND)),
                Span::styled(hunk_text, styles::style_hunk_header()),
                Span::styled(" ".repeat(padding_len), Style::default().bg(styles::BG_HUNK_HEADER)),
                Span::styled(styles::BORDER_VERTICAL, border_style),
            ])
        }
        LineContent::SplitLine {
            old_num,
            old_segments,
            new_num,
            new_segments,
        } => {
            let side_content_width = half_width.saturating_sub(7);

            let old_num_str = old_num.map(|n| format!("{:>4} ", n)).unwrap_or_else(|| "     ".into());
            let new_num_str = new_num.map(|n| format!("{:>4} ", n)).unwrap_or_else(|| "     ".into());

            // Determine backgrounds based on content presence and word diffs
            let has_old_changes = old_segments.iter().any(|s| s.is_changed);
            let has_new_changes = new_segments.iter().any(|s| s.is_changed);

            let (old_margin_bg, old_line_bg, old_word_bg): (Option<Color>, Option<Color>, Option<Color>) = if !old_segments.is_empty() && new_segments.is_empty() {
                (Some(styles::BG_DELETION_MARGIN), Some(styles::BG_DELETION_LINE), Some(styles::BG_DELETION_WORD))
            } else if has_old_changes {
                (Some(styles::BG_DELETION_MARGIN), Some(styles::BG_DELETION_LINE), Some(styles::BG_DELETION_WORD))
            } else {
                (None, None, None) // No background for context
            };

            let (new_margin_bg, new_line_bg, new_word_bg): (Option<Color>, Option<Color>, Option<Color>) = if !new_segments.is_empty() && old_segments.is_empty() {
                (Some(styles::BG_ADDITION_MARGIN), Some(styles::BG_ADDITION_LINE), Some(styles::BG_ADDITION_WORD))
            } else if has_new_changes {
                (Some(styles::BG_ADDITION_MARGIN), Some(styles::BG_ADDITION_LINE), Some(styles::BG_ADDITION_WORD))
            } else {
                (None, None, None) // No background for context
            };

            let mut spans = Vec::with_capacity(old_segments.len() + new_segments.len() + 8);

            // Left border and old line number
            spans.push(Span::styled(styles::BORDER_VERTICAL, border_style));
            let old_num_style = if let Some(bg) = old_margin_bg {
                Style::default().fg(styles::FG_LINE_NUM).bg(bg)
            } else {
                Style::default().fg(styles::FG_LINE_NUM)
            };
            spans.push(Span::styled(old_num_str, old_num_style));

            // Old content
            let mut char_count = 0;
            for seg in old_segments {
                if char_count >= side_content_width {
                    break;
                }
                let bg = if seg.is_changed { old_word_bg } else { old_line_bg };
                let mut style = Style::default().fg(seg.fg);
                if let Some(bg_color) = bg {
                    style = style.bg(bg_color);
                }
                if seg.bold {
                    style = style.add_modifier(ratatui::style::Modifier::BOLD);
                }
                if seg.italic {
                    style = style.add_modifier(ratatui::style::Modifier::ITALIC);
                }
                let seg_chars = seg.text.chars().count();
                if char_count + seg_chars > side_content_width {
                    let take = side_content_width - char_count;
                    let truncated: String = seg.text.chars().take(take).collect();
                    spans.push(Span::styled(truncated, style));
                    char_count = side_content_width;
                } else {
                    spans.push(Span::styled(seg.text.clone(), style));
                    char_count += seg_chars;
                }
            }
            if char_count < side_content_width {
                let pad_style = if let Some(bg) = old_line_bg {
                    Style::default().bg(bg)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(" ".repeat(side_content_width - char_count), pad_style));
            }

            // Middle divider
            spans.push(Span::styled(" │ ", styles::style_muted()));

            // New line number
            let new_num_style = if let Some(bg) = new_margin_bg {
                Style::default().fg(styles::FG_LINE_NUM).bg(bg)
            } else {
                Style::default().fg(styles::FG_LINE_NUM)
            };
            spans.push(Span::styled(new_num_str, new_num_style));

            // New content
            char_count = 0;
            for seg in new_segments {
                if char_count >= side_content_width {
                    break;
                }
                let bg = if seg.is_changed { new_word_bg } else { new_line_bg };
                let mut style = Style::default().fg(seg.fg);
                if let Some(bg_color) = bg {
                    style = style.bg(bg_color);
                }
                if seg.bold {
                    style = style.add_modifier(ratatui::style::Modifier::BOLD);
                }
                if seg.italic {
                    style = style.add_modifier(ratatui::style::Modifier::ITALIC);
                }
                let seg_chars = seg.text.chars().count();
                if char_count + seg_chars > side_content_width {
                    let take = side_content_width - char_count;
                    let truncated: String = seg.text.chars().take(take).collect();
                    spans.push(Span::styled(truncated, style));
                    char_count = side_content_width;
                } else {
                    spans.push(Span::styled(seg.text.clone(), style));
                    char_count += seg_chars;
                }
            }
            if char_count < side_content_width {
                let pad_style = if let Some(bg) = new_line_bg {
                    Style::default().bg(bg)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(" ".repeat(side_content_width - char_count), pad_style));
            }

            spans.push(Span::styled(styles::BORDER_VERTICAL, border_style));
            Line::from(spans)
        }
        LineContent::Empty => {
            Line::from(vec![Span::raw(" ".repeat(w))]) // No background
        }
        _ => Line::from(""),
    }
}

fn render_scrollbar(frame: &mut Frame, area: Rect, total_lines: usize, scroll: usize) {
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some(" "))
        .thumb_symbol("█");
    let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll);
    frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}

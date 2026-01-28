//! Main diff view component with GitHub-style file blocks, word-level diffs, and split view.
//! Performance optimized: syntax highlighting is cached during build, not render.

#![allow(dead_code)]

use crate::app::DiffSource;
use crate::domain::{Diff, DiffLine, DiffStats};
use crate::ui::{styles, syntax};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
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

/// Render unified diff view.
pub fn render_unified(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    lines: &[DiffViewLine],
    scroll: usize,
    current_file: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    diff_source: DiffSource,
    uncommitted_files: &HashSet<String>,
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
        let sticky_line = render_sticky_header(&path, &stats, file_idx, current_file, collapsed, is_viewed, sticky_rect.width);
        let sticky_para = Paragraph::new(vec![sticky_line]);
        frame.render_widget(sticky_para, sticky_rect);
    }

    let visible_height = content_area.height as usize;

    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(scroll)
        .take(visible_height)
        .map(|line| {
            let mut rendered = render_unified_line(line, current_file, collapsed, viewed, content_width);

            // Determine if this line's file has uncommitted changes
            let file_path = diff.files.get(line.file_index).map(|f| &f.path);
            let is_uncommitted = file_path
                .map(|p| uncommitted_files.contains(p))
                .unwrap_or(false);

            // Prepend gutter (always present for consistent layout)
            let gutter_style = if diff_source == DiffSource::All && is_uncommitted {
                Style::default().fg(styles::FG_WARNING)
            } else {
                Style::default().fg(styles::FG_BORDER)
            };

            let gutter_char = if diff_source == DiffSource::All && is_uncommitted {
                "▎ " // Orange bar for uncommitted
            } else {
                "  " // Empty gutter
            };

            let mut spans = vec![Span::styled(gutter_char, gutter_style)];
            spans.extend(rendered.spans.drain(..));
            Line::from(spans)
        })
        .collect();

    let content = Paragraph::new(visible_lines);
    frame.render_widget(content, content_area);

    if lines.len() > visible_height {
        render_scrollbar(frame, content_area, lines.len(), scroll);
    }
}

/// Render split diff view.
pub fn render_split(
    frame: &mut Frame,
    area: Rect,
    diff: &Diff,
    lines: &[DiffViewLine],
    scroll: usize,
    current_file: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    diff_source: DiffSource,
    uncommitted_files: &HashSet<String>,
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
        let sticky_line = render_sticky_header(&path, &stats, file_idx, current_file, collapsed, is_viewed, area.width);
        let sticky_para = Paragraph::new(vec![sticky_line]);
        frame.render_widget(sticky_para, area);
    }

    let half_width = content_area.width.saturating_sub(gutter_width) / 2;
    let visible_height = content_area.height as usize;

    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(scroll)
        .take(visible_height)
        .map(|line| {
            let mut rendered = render_split_line(line, current_file, collapsed, viewed, half_width as usize, content_area.width.saturating_sub(gutter_width));

            // Determine if this line's file has uncommitted changes
            let file_path = diff.files.get(line.file_index).map(|f| &f.path);
            let is_uncommitted = file_path
                .map(|p| uncommitted_files.contains(p))
                .unwrap_or(false);

            // Prepend gutter (always present for consistent layout)
            let gutter_style = if diff_source == DiffSource::All && is_uncommitted {
                Style::default().fg(styles::FG_WARNING)
            } else {
                Style::default().fg(styles::FG_BORDER)
            };

            let gutter_char = if diff_source == DiffSource::All && is_uncommitted {
                "▎ " // Orange bar for uncommitted
            } else {
                "  " // Empty gutter
            };

            let mut spans = vec![Span::styled(gutter_char, gutter_style)];
            spans.extend(rendered.spans.drain(..));
            Line::from(spans)
        })
        .collect();

    let content = Paragraph::new(visible_lines);
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

/// Render a sticky file header.
fn render_sticky_header(
    path: &str,
    stats: &DiffStats,
    file_index: usize,
    current_file: usize,
    collapsed: &HashSet<usize>,
    is_viewed: bool,
    width: u16,
) -> Line<'static> {
    let w = width as usize;
    let is_collapsed = collapsed.contains(&file_index);
    let is_current = file_index == current_file;

    let toggle = if is_collapsed { "▶" } else { "▼" };
    let checkbox = if is_viewed { "☑" } else { "☐" };
    let border_style = if is_current { styles::style_border_selected() } else { styles::style_border() };
    let header_bg = styles::BG_FILE_HEADER; // Sticky header always uses standard bg

    let total = stats.additions + stats.deletions;
    let bar_width = 5;
    let add_chars = if total > 0 { (stats.additions * bar_width / total).min(bar_width) } else { 0 };
    let del_chars = bar_width - add_chars;
    let add_bar: String = "█".repeat(add_chars);
    let del_bar: String = "█".repeat(del_chars);

    let stats_display = format!("+{} -{} ", stats.additions, stats.deletions);
    let right_len = stats_display.len() + bar_width + 4; // +4 for checkbox
    let left_content = format!(" {} {}", toggle, path);
    let left_len = left_content.len();
    let inner_width = w.saturating_sub(2);
    let padding_len = inner_width.saturating_sub(left_len + right_len);

    Line::from(vec![
        Span::styled(styles::BORDER_VERTICAL, border_style),
        Span::styled(left_content, Style::default().fg(styles::FG_PATH).bg(header_bg)),
        Span::styled(" ".repeat(padding_len), Style::default().bg(header_bg)),
        Span::styled(checkbox, Style::default().fg(if is_viewed { styles::FG_ADDITION } else { styles::FG_MUTED }).bg(header_bg)),
        Span::styled(" ", Style::default().bg(header_bg)),
        Span::styled(stats_display, Style::default().fg(styles::FG_DEFAULT).bg(header_bg)),
        Span::styled(add_bar, styles::style_stat_addition().bg(header_bg)),
        Span::styled(del_bar, styles::style_stat_deletion().bg(header_bg)),
        Span::styled(" ", Style::default().bg(header_bg)),
        Span::styled(styles::BORDER_VERTICAL, border_style),
    ])
}

#[inline]
fn render_unified_line(
    line: &DiffViewLine,
    current_file: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    width: u16,
) -> Line<'static> {
    let w = width as usize;

    match &line.content {
        LineContent::FileHeaderTop { path, stats } => {
            let is_viewed = viewed.contains(&line.file_index);
            render_file_header_top(path, stats, line.file_index, current_file, collapsed, is_viewed, width)
        }
        LineContent::FileHeaderBottom => {
            render_file_header_bottom(line.file_index, current_file, width)
        }
        LineContent::HunkHeader { text } => {
            let inner_width = w.saturating_sub(2);
            let expand_area = "  ⋯  ";
            let hunk_text = format!(" {} ", text);
            let used_width = expand_area.len() + hunk_text.len();
            let padding_len = inner_width.saturating_sub(used_width);

            Line::from(vec![
                Span::styled(styles::BORDER_VERTICAL, styles::style_border()),
                Span::styled(expand_area, Style::default().fg(styles::FG_HUNK).bg(styles::BG_HUNK_EXPAND)),
                Span::styled(hunk_text, styles::style_hunk_header()),
                Span::styled(" ".repeat(padding_len), Style::default().bg(styles::BG_HUNK_HEADER)),
                Span::styled(styles::BORDER_VERTICAL, styles::style_border()),
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
            spans.push(Span::styled(styles::BORDER_VERTICAL, styles::style_border()));

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

            spans.push(Span::styled(styles::BORDER_VERTICAL, styles::style_border()));
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
    width: u16,
) -> Line<'static> {
    let w = width as usize;
    let is_collapsed = collapsed.contains(&file_index);
    let is_current = file_index == current_file;

    let toggle = if is_collapsed { "▶" } else { "▼" };
    let checkbox = if is_viewed { "☑" } else { "☐" };
    let border_style = if is_current { styles::style_border_selected() } else { styles::style_border() };
    let header_bg = if is_current { styles::BG_SELECTED } else { styles::BG_FILE_HEADER };

    let total = stats.additions + stats.deletions;
    let bar_width = 5;
    let add_chars = if total > 0 { (stats.additions * bar_width / total).min(bar_width) } else { 0 };
    let del_chars = bar_width - add_chars;
    let add_bar: String = "█".repeat(add_chars);
    let del_bar: String = "█".repeat(del_chars);

    let stats_display = format!("+{} -{} ", stats.additions, stats.deletions);
    let right_len = stats_display.len() + bar_width + 4; // +4 for checkbox and space
    let left_content = format!(" {} {}", toggle, path);
    let left_len = left_content.len();
    let inner_width = w.saturating_sub(2);
    let padding_len = inner_width.saturating_sub(left_len + right_len);

    Line::from(vec![
        Span::styled(styles::BORDER_TOP_LEFT, border_style),
        Span::styled(left_content, Style::default().fg(styles::FG_PATH).bg(header_bg)),
        Span::styled(" ".repeat(padding_len), Style::default().bg(header_bg)),
        Span::styled(checkbox, Style::default().fg(if is_viewed { styles::FG_ADDITION } else { styles::FG_MUTED }).bg(header_bg)),
        Span::styled(" ", Style::default().bg(header_bg)),
        Span::styled(stats_display, Style::default().fg(styles::FG_DEFAULT).bg(header_bg)),
        Span::styled(add_bar, styles::style_stat_addition().bg(header_bg)),
        Span::styled(del_bar, styles::style_stat_deletion().bg(header_bg)),
        Span::styled(" ", Style::default().bg(header_bg)),
        Span::styled(styles::BORDER_TOP_RIGHT, border_style),
    ])
}

fn render_file_header_bottom(file_index: usize, current_file: usize, width: u16) -> Line<'static> {
    let w = width as usize;
    let is_current = file_index == current_file;
    let border_style = if is_current { styles::style_border_selected() } else { styles::style_border() };

    let inner_width = w.saturating_sub(2);
    let border_line = styles::BORDER_HORIZONTAL.repeat(inner_width);

    Line::from(vec![
        Span::styled(styles::BORDER_BOTTOM_LEFT, border_style),
        Span::styled(border_line, border_style),
        Span::styled(styles::BORDER_BOTTOM_RIGHT, border_style),
    ])
}

#[inline]
fn render_split_line(
    line: &DiffViewLine,
    current_file: usize,
    collapsed: &HashSet<usize>,
    viewed: &HashSet<usize>,
    half_width: usize,
    full_width: u16,
) -> Line<'static> {
    let w = full_width as usize;

    match &line.content {
        LineContent::FileHeaderTop { path, stats } => {
            let is_viewed = viewed.contains(&line.file_index);
            render_file_header_top(path, stats, line.file_index, current_file, collapsed, is_viewed, full_width)
        }
        LineContent::FileHeaderBottom => {
            render_file_header_bottom(line.file_index, current_file, full_width)
        }
        LineContent::HunkHeader { text } => {
            let inner_width = w.saturating_sub(2);
            let expand_area = "  ⋯  ";
            let hunk_text = format!(" {} ", text);
            let used_width = expand_area.len() + hunk_text.len();
            let padding_len = inner_width.saturating_sub(used_width);

            Line::from(vec![
                Span::styled(styles::BORDER_VERTICAL, styles::style_border()),
                Span::styled(expand_area, Style::default().fg(styles::FG_HUNK).bg(styles::BG_HUNK_EXPAND)),
                Span::styled(hunk_text, styles::style_hunk_header()),
                Span::styled(" ".repeat(padding_len), Style::default().bg(styles::BG_HUNK_HEADER)),
                Span::styled(styles::BORDER_VERTICAL, styles::style_border()),
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
            spans.push(Span::styled(styles::BORDER_VERTICAL, styles::style_border()));
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

            spans.push(Span::styled(styles::BORDER_VERTICAL, styles::style_border()));
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

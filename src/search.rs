//! Fuzzy search through diff content.

use crate::ui::diff_view::{DiffViewLine, LineContent, LineKind};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};

/// A searchable entry in the diff index.
#[derive(Debug, Clone)]
pub struct SearchableEntry {
    pub file_path: String,
    pub diff_line_index: usize,
    pub line_kind: LineKind,
    pub line_number: Option<u32>,
    pub content: String,
}

/// A search result with score and match indices.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: SearchableEntry,
    pub score: u32,
    pub match_indices: Vec<u32>,
}

/// State for fuzzy search modal.
#[derive(Debug, Clone, Default)]
pub struct FuzzySearchState {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub selected_index: usize,
    pub scroll: usize,
}

impl FuzzySearchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.results.len() - 1);
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Ensure selected item is visible in the list.
    pub fn ensure_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.selected_index < self.scroll {
            self.scroll = self.selected_index;
        } else if self.selected_index >= self.scroll + visible_height {
            self.scroll = self.selected_index + 1 - visible_height;
        }
    }

    /// Get the currently selected result.
    pub fn selected(&self) -> Option<&SearchResult> {
        self.results.get(self.selected_index)
    }
}

/// Build search index from diff lines.
pub fn build_search_index(diff_lines: &[DiffViewLine], file_paths: &[String]) -> Vec<SearchableEntry> {
    let mut entries = Vec::with_capacity(diff_lines.len());

    for (idx, line) in diff_lines.iter().enumerate() {
        // Only index lines with actual content
        let content = match &line.content {
            LineContent::UnifiedLine { segments, new_num, .. } => {
                let text: String = segments.iter().map(|s| s.text.as_str()).collect();
                if text.trim().is_empty() {
                    continue;
                }
                (text, *new_num)
            }
            LineContent::SplitLine { old_segments, new_segments, new_num, .. } => {
                // Prefer new content, fall back to old
                let text: String = if !new_segments.is_empty() {
                    new_segments.iter().map(|s| s.text.as_str()).collect()
                } else {
                    old_segments.iter().map(|s| s.text.as_str()).collect()
                };
                if text.trim().is_empty() {
                    continue;
                }
                (text, *new_num)
            }
            LineContent::HunkHeader { text } => {
                (text.clone(), None)
            }
            LineContent::FileHeaderTop { path, .. } => {
                (path.clone(), None)
            }
            _ => continue,
        };

        let file_path = file_paths.get(line.file_index).cloned().unwrap_or_default();

        entries.push(SearchableEntry {
            file_path,
            diff_line_index: idx,
            line_kind: line.kind,
            line_number: content.1,
            content: content.0,
        });
    }

    entries
}

/// Perform fuzzy search on the index.
pub fn fuzzy_search(query: &str, index: &[SearchableEntry], max_results: usize) -> Vec<SearchResult> {
    if query.is_empty() {
        return Vec::new();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);

    let mut results: Vec<SearchResult> = Vec::new();

    for entry in index {
        let mut buf = Vec::new();
        let haystack = Utf32Str::new(&entry.content, &mut buf);
        let mut indices = Vec::new();
        if let Some(score) = pattern.indices(haystack, &mut matcher, &mut indices) {
            results.push(SearchResult {
                entry: entry.clone(),
                score,
                match_indices: indices,
            });
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.score.cmp(&a.score));

    // Limit results
    results.truncate(max_results);

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::diff_view::HighlightedSegment;
    use ratatui::style::Color;

    fn make_segment(text: &str) -> HighlightedSegment {
        HighlightedSegment {
            text: text.to_string(),
            fg: Color::White,
            bold: false,
            italic: false,
            is_changed: false,
        }
    }

    #[test]
    fn test_build_search_index() {
        let diff_lines = vec![
            DiffViewLine {
                kind: LineKind::Addition,
                file_index: 0,
                content: LineContent::UnifiedLine {
                    old_num: None,
                    new_num: Some(1),
                    prefix: '+',
                    segments: vec![make_segment("fn main() {")],
                },
            },
            DiffViewLine {
                kind: LineKind::Context,
                file_index: 0,
                content: LineContent::UnifiedLine {
                    old_num: Some(2),
                    new_num: Some(2),
                    prefix: ' ',
                    segments: vec![make_segment("    println!(\"hello\");")],
                },
            },
        ];

        let file_paths = vec!["src/main.rs".to_string()];
        let index = build_search_index(&diff_lines, &file_paths);

        assert_eq!(index.len(), 2);
        assert_eq!(index[0].content, "fn main() {");
        assert_eq!(index[1].content, "    println!(\"hello\");");
    }

    #[test]
    fn test_fuzzy_search() {
        let index = vec![
            SearchableEntry {
                file_path: "src/main.rs".to_string(),
                diff_line_index: 0,
                line_kind: LineKind::Addition,
                line_number: Some(1),
                content: "fn main() {".to_string(),
            },
            SearchableEntry {
                file_path: "src/lib.rs".to_string(),
                diff_line_index: 1,
                line_kind: LineKind::Addition,
                line_number: Some(5),
                content: "pub fn helper() {".to_string(),
            },
        ];

        let results = fuzzy_search("main", &index, 100);
        assert!(!results.is_empty());
        assert!(results[0].entry.content.contains("main"));
    }

    #[test]
    fn test_empty_query() {
        let index = vec![SearchableEntry {
            file_path: "test.rs".to_string(),
            diff_line_index: 0,
            line_kind: LineKind::Context,
            line_number: Some(1),
            content: "some content".to_string(),
        }];

        let results = fuzzy_search("", &index, 100);
        assert!(results.is_empty());
    }
}

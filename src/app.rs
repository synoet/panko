//! Application state machine.

use crate::domain::{BranchPreview, Diff};
use crate::ports::{GitRepo, KeyCode, KeyModifiers, Terminal, TerminalEvent};
use crate::ui::{diff_view, file_tree, layout};
use anyhow::Result;
use ratatui::widgets::ListState;
use std::time::Duration;

/// Current view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Normal,
    Help,
}

/// Which pane has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    FileTree,
    DiffView,
}

/// Application state.
pub struct App {
    pub preview: BranchPreview,
    pub diff: Diff,
    pub flat_items: Vec<file_tree::FlatItem>,
    pub diff_lines: Vec<diff_view::DiffViewLine>,
    pub mode: ViewMode,
    pub focus: Focus,
    pub selected_tree_item: usize,
    pub current_file_index: usize,
    pub scroll: usize,
    pub should_quit: bool,
    pub tree_state: ListState,
}

impl App {
    pub fn new(git: &dyn GitRepo, base_override: Option<&str>) -> Result<Self> {
        let current_branch = git.current_branch()?;
        let base_branch = base_override
            .map(String::from)
            .unwrap_or_else(|| git.detect_base_branch().unwrap_or_else(|_| "main".to_string()));

        let merge_base = git.merge_base(&base_branch)?;
        let commits = git.commits_since(&merge_base)?;
        let diff = git.diff_to_base(&merge_base)?;

        // Build UI data structures
        let tree_nodes = file_tree::build_tree(&diff);
        let flat_items = file_tree::flatten_tree(&tree_nodes);
        let diff_lines = diff_view::build_diff_lines(&diff);

        Ok(Self {
            preview: BranchPreview {
                current_branch,
                base_branch,
                merge_base,
                commits,
            },
            diff,
            flat_items,
            diff_lines,
            mode: ViewMode::Normal,
            focus: Focus::DiffView,
            selected_tree_item: 0,
            current_file_index: 0,
            scroll: 0,
            should_quit: false,
            tree_state: ListState::default(),
        })
    }

    pub fn run<T: Terminal>(&mut self, terminal: &mut T, _git: &dyn GitRepo) -> Result<()> {
        while !self.should_quit {
            self.draw(terminal)?;

            if let Some(event) = terminal.poll_event(Duration::from_millis(50))? {
                self.handle_event(event)?;
            }
        }
        Ok(())
    }

    fn draw<T: Terminal>(&mut self, terminal: &mut T) -> Result<()> {
        let diff = &self.diff;
        let flat_items = &self.flat_items;
        let diff_lines = &self.diff_lines;
        let selected_tree_item = self.selected_tree_item;
        let current_file_index = self.current_file_index;
        let scroll = self.scroll;
        let branch = &self.preview.current_branch;
        let base = &self.preview.base_branch;
        let mode = self.mode;
        let tree_state = &mut self.tree_state;

        terminal.draw(|frame| {
            let area = frame.area();

            if diff.files.is_empty() {
                layout::render_empty(frame, area, "No changes found", branch, base);
            } else {
                layout::render_main(
                    frame,
                    area,
                    diff,
                    flat_items,
                    diff_lines,
                    selected_tree_item,
                    current_file_index,
                    scroll,
                    branch,
                    base,
                    tree_state,
                );
            }

            if mode == ViewMode::Help {
                layout::render_help(frame, area);
            }
        })
    }

    fn handle_event(&mut self, event: TerminalEvent) -> Result<()> {
        match event {
            TerminalEvent::Key(key) => self.handle_key(key.code, key.modifiers),
            TerminalEvent::Resize(_, _) => Ok(()),
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        // Global keys
        if code == KeyCode::Char('q') && !modifiers.ctrl {
            self.should_quit = true;
            return Ok(());
        }

        if code == KeyCode::Char('c') && modifiers.ctrl {
            self.should_quit = true;
            return Ok(());
        }

        // Help mode
        if self.mode == ViewMode::Help {
            self.mode = ViewMode::Normal;
            return Ok(());
        }

        if code == KeyCode::Char('?') {
            self.mode = ViewMode::Help;
            return Ok(());
        }

        // Tab to switch focus
        if code == KeyCode::Tab {
            self.focus = match self.focus {
                Focus::FileTree => Focus::DiffView,
                Focus::DiffView => Focus::FileTree,
            };
            return Ok(());
        }

        match self.focus {
            Focus::FileTree => self.handle_tree_key(code),
            Focus::DiffView => self.handle_diff_key(code),
        }
    }

    fn handle_tree_key(&mut self, code: KeyCode) -> Result<()> {
        let max_item = self.flat_items.len().saturating_sub(1);

        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_tree_item < max_item {
                    self.selected_tree_item += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected_tree_item = self.selected_tree_item.saturating_sub(1);
            }
            KeyCode::Enter => {
                if let Some(item) = self.flat_items.get(self.selected_tree_item) {
                    if let Some(file_idx) = item.file_index {
                        self.current_file_index = file_idx;
                        self.scroll = diff_view::find_file_start(&self.diff_lines, file_idx);
                        self.focus = Focus::DiffView;
                    }
                }
            }
            KeyCode::Char('g') => {
                self.selected_tree_item = 0;
            }
            KeyCode::Char('G') => {
                self.selected_tree_item = max_item;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_diff_key(&mut self, code: KeyCode) -> Result<()> {
        let max_scroll = self.diff_lines.len().saturating_sub(1);
        let file_count = self.diff.files.len();

        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                if self.scroll < max_scroll {
                    self.scroll += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.scroll = (self.scroll + 20).min(max_scroll);
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(20);
            }
            KeyCode::Char('n') => {
                // Next file
                if self.current_file_index < file_count.saturating_sub(1) {
                    self.current_file_index += 1;
                    self.scroll = diff_view::find_file_start(&self.diff_lines, self.current_file_index);
                    self.sync_tree_selection();
                }
            }
            KeyCode::Char('p') => {
                // Previous file
                if self.current_file_index > 0 {
                    self.current_file_index -= 1;
                    self.scroll = diff_view::find_file_start(&self.diff_lines, self.current_file_index);
                    self.sync_tree_selection();
                }
            }
            KeyCode::Char('g') => {
                self.scroll = 0;
                self.current_file_index = 0;
                self.sync_tree_selection();
            }
            KeyCode::Char('G') => {
                self.scroll = max_scroll;
                self.current_file_index = file_count.saturating_sub(1);
                self.sync_tree_selection();
            }
            _ => {}
        }
        Ok(())
    }

    fn sync_tree_selection(&mut self) {
        // Find the flat_items index that corresponds to current_file_index
        for (i, item) in self.flat_items.iter().enumerate() {
            if item.file_index == Some(self.current_file_index) {
                self.selected_tree_item = i;
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Commit, DiffLine, DiffStats, FileDiff, Hunk};

    struct FakeGitRepo {
        branch: String,
        base: String,
        merge_base: String,
        commits: Vec<Commit>,
        diff: Diff,
    }

    impl FakeGitRepo {
        fn new() -> Self {
            Self {
                branch: "feature".to_string(),
                base: "main".to_string(),
                merge_base: "abc123".to_string(),
                commits: vec![
                    Commit {
                        hash: "def456".to_string(),
                        short_hash: "def456".to_string(),
                        message: "Add feature".to_string(),
                        author: "Test".to_string(),
                        email: "test@example.com".to_string(),
                        timestamp: 0,
                    },
                ],
                diff: Diff {
                    files: vec![FileDiff {
                        path: "src/main.rs".to_string(),
                        old_path: None,
                        hunks: vec![Hunk {
                            old_start: 1,
                            old_lines: 3,
                            new_start: 1,
                            new_lines: 4,
                            lines: vec![
                                DiffLine::Context("fn main() {".to_string()),
                                DiffLine::Addition("    println!(\"hello\");".to_string()),
                                DiffLine::Context("}".to_string()),
                            ],
                        }],
                        stats: DiffStats::new(1, 0),
                        is_binary: false,
                    }],
                },
            }
        }
    }

    impl GitRepo for FakeGitRepo {
        fn current_branch(&self) -> Result<String> {
            Ok(self.branch.clone())
        }

        fn detect_base_branch(&self) -> Result<String> {
            Ok(self.base.clone())
        }

        fn merge_base(&self, _base: &str) -> Result<String> {
            Ok(self.merge_base.clone())
        }

        fn commits_since(&self, _merge_base: &str) -> Result<Vec<Commit>> {
            Ok(self.commits.clone())
        }

        fn diff_to_base(&self, _merge_base: &str) -> Result<Diff> {
            Ok(self.diff.clone())
        }

        fn commit_diff(&self, _hash: &str) -> Result<Diff> {
            Ok(self.diff.clone())
        }
    }

    #[test]
    fn test_app_initializes() {
        let git = FakeGitRepo::new();
        let app = App::new(&git, None).unwrap();
        assert_eq!(app.mode, ViewMode::Normal);
        assert!(!app.diff.files.is_empty());
    }

    #[test]
    fn test_navigate_diff() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();
        app.focus = Focus::DiffView;
        assert_eq!(app.scroll, 0);

        app.handle_key(KeyCode::Down, KeyModifiers::default()).unwrap();
        assert_eq!(app.scroll, 1);

        app.handle_key(KeyCode::Up, KeyModifiers::default()).unwrap();
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn test_q_quits() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();
        assert!(!app.should_quit);

        app.handle_key(KeyCode::Char('q'), KeyModifiers::default()).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_help_toggle() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();

        app.handle_key(KeyCode::Char('?'), KeyModifiers::default()).unwrap();
        assert_eq!(app.mode, ViewMode::Help);

        app.handle_key(KeyCode::Esc, KeyModifiers::default()).unwrap();
        assert_eq!(app.mode, ViewMode::Normal);
    }
}

//! Application state machine.

use crate::domain::{BranchPreview, Diff};
use crate::ports::{
    FileWatcher, GitRepo, KeyCode, KeyModifiers, MouseEvent, StateStore, Terminal, TerminalEvent,
};
use crate::ui::{diff_view, file_tree, layout};
use anyhow::Result;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

/// Current view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Normal,
    Help,
}

/// Which pane/input has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    FileTree,
    #[default]
    DiffView,
    FilterInput,
}

/// Application state.
pub struct App {
    pub preview: BranchPreview,
    pub diff: Diff,
    pub tree_nodes: Vec<file_tree::TreeNode>,
    pub flat_items: Vec<file_tree::FlatItem>,
    pub diff_lines: Vec<diff_view::DiffViewLine>,
    pub mode: ViewMode,
    pub focus: Focus,
    pub diff_view_mode: diff_view::DiffViewMode,
    pub selected_tree_item: usize,
    pub current_file_index: usize,
    pub scroll: usize,
    pub collapsed_files: HashSet<usize>,
    pub viewed_files: HashSet<usize>,
    pub filter: String,
    pub should_quit: bool,
    pub tree_state: ListState,
    pub sidebar_collapsed: bool,

    // State persistence and file watching
    repo_path: String,
    branch: String,
    state_store: Option<Arc<dyn StateStore>>,
    file_watcher: Option<Box<dyn FileWatcher>>,
    pub has_pending_changes: bool,
    // Map from file path to viewed_at timestamp (for "new changes" detection)
    viewed_timestamps: HashMap<String, i64>,
}

impl App {
    pub fn new(
        git: &dyn GitRepo,
        base_override: Option<&str>,
        state_store: Option<Arc<dyn StateStore>>,
        file_watcher: Option<Box<dyn FileWatcher>>,
    ) -> Result<Self> {
        let current_branch = git.current_branch()?;
        let repo_path = git.repo_path()?;
        let base_branch = base_override
            .map(String::from)
            .unwrap_or_else(|| git.detect_base_branch().unwrap_or_else(|_| "main".to_string()));

        let merge_base = git.merge_base(&base_branch)?;
        let commits = git.commits_since(&merge_base)?;
        let diff = git.diff_to_base(&merge_base)?;

        // Build UI data structures
        let tree_nodes = file_tree::build_tree(&diff);
        let flat_items = file_tree::flatten_tree(&tree_nodes, "");
        let collapsed_files = HashSet::new();
        let diff_lines = diff_view::build_unified_lines(&diff, &collapsed_files);

        // Load viewed files from state store
        let (viewed_files, viewed_timestamps) =
            Self::load_viewed_state(&state_store, &repo_path, &current_branch, &diff);

        Ok(Self {
            preview: BranchPreview {
                current_branch: current_branch.clone(),
                base_branch,
                merge_base,
                commits,
            },
            diff,
            tree_nodes,
            flat_items,
            diff_lines,
            mode: ViewMode::Normal,
            focus: Focus::DiffView,
            diff_view_mode: diff_view::DiffViewMode::Unified,
            selected_tree_item: 0,
            current_file_index: 0,
            scroll: 0,
            collapsed_files,
            viewed_files,
            filter: String::new(),
            should_quit: false,
            tree_state: ListState::default(),
            sidebar_collapsed: false,
            repo_path,
            branch: current_branch,
            state_store,
            file_watcher,
            has_pending_changes: false,
            viewed_timestamps,
        })
    }

    /// Load viewed state from the state store, returning (viewed_files set by index, timestamps map).
    fn load_viewed_state(
        state_store: &Option<Arc<dyn StateStore>>,
        repo_path: &str,
        branch: &str,
        diff: &Diff,
    ) -> (HashSet<usize>, HashMap<String, i64>) {
        let mut viewed_files = HashSet::new();
        let mut viewed_timestamps = HashMap::new();

        if let Some(store) = state_store {
            if let Ok(files) = store.get_viewed_files(repo_path, branch) {
                // Build a map of file path -> index in diff
                let path_to_index: HashMap<&str, usize> = diff
                    .files
                    .iter()
                    .enumerate()
                    .map(|(i, f)| (f.path.as_str(), i))
                    .collect();

                for viewed in files {
                    if let Some(&idx) = path_to_index.get(viewed.file_path.as_str()) {
                        viewed_files.insert(idx);
                    }
                    viewed_timestamps.insert(viewed.file_path, viewed.viewed_at);
                }
            }
        }

        (viewed_files, viewed_timestamps)
    }

    pub fn run<T: Terminal>(&mut self, terminal: &mut T, git: &dyn GitRepo) -> Result<()> {
        while !self.should_quit {
            // Check for file changes
            if let Some(ref watcher) = self.file_watcher {
                if watcher.has_changes() {
                    self.has_pending_changes = true;
                }
            }

            self.draw(terminal)?;

            if let Some(event) = terminal.poll_event(Duration::from_millis(50))? {
                self.handle_event(event, git)?;
            }
        }
        Ok(())
    }

    /// Refresh git data (reload diff and commits).
    fn refresh(&mut self, git: &dyn GitRepo) -> Result<()> {
        let merge_base = git.merge_base(&self.preview.base_branch)?;
        let commits = git.commits_since(&merge_base)?;
        let diff = git.diff_to_base(&merge_base)?;

        // Rebuild UI data structures
        self.tree_nodes = file_tree::build_tree(&diff);
        self.flat_items = file_tree::flatten_tree(&self.tree_nodes, &self.filter);
        self.diff_lines = diff_view::build_unified_lines(&diff, &self.collapsed_files);

        // Reload viewed state (in case files changed)
        let (viewed_files, viewed_timestamps) =
            Self::load_viewed_state(&self.state_store, &self.repo_path, &self.branch, &diff);

        self.preview.merge_base = merge_base;
        self.preview.commits = commits;
        self.diff = diff;
        self.viewed_files = viewed_files;
        self.viewed_timestamps = viewed_timestamps;

        // Clear pending changes flag and watcher
        self.has_pending_changes = false;
        if let Some(ref watcher) = self.file_watcher {
            watcher.clear_changes();
        }

        // Clamp indices
        if self.current_file_index >= self.diff.files.len() {
            self.current_file_index = self.diff.files.len().saturating_sub(1);
        }
        if self.scroll >= self.diff_lines.len() {
            self.scroll = self.diff_lines.len().saturating_sub(1);
        }

        Ok(())
    }

    fn rebuild_diff_lines(&mut self) {
        self.diff_lines = match self.diff_view_mode {
            diff_view::DiffViewMode::Unified => {
                diff_view::build_unified_lines(&self.diff, &self.collapsed_files)
            }
            diff_view::DiffViewMode::Split => {
                diff_view::build_split_lines(&self.diff, &self.collapsed_files)
            }
        };
    }

    fn rebuild_flat_items(&mut self) {
        self.flat_items = file_tree::flatten_tree(&self.tree_nodes, &self.filter);
        // Ensure selected item is in bounds
        if self.selected_tree_item >= self.flat_items.len() {
            self.selected_tree_item = self.flat_items.len().saturating_sub(1);
        }
    }

    fn draw<T: Terminal>(&mut self, terminal: &mut T) -> Result<()> {
        let diff = &self.diff;
        let flat_items = &self.flat_items;
        let diff_lines = &self.diff_lines;
        let selected_tree_item = self.selected_tree_item;
        let current_file_index = self.current_file_index;
        let scroll = self.scroll;
        let collapsed = &self.collapsed_files;
        let viewed = &self.viewed_files;
        let filter = &self.filter;
        let filter_focused = self.focus == Focus::FilterInput;
        let view_mode = self.diff_view_mode;
        let branch = &self.preview.current_branch;
        let base = &self.preview.base_branch;
        let mode = self.mode;
        let tree_state = &mut self.tree_state;
        let sidebar_collapsed = self.sidebar_collapsed;
        let has_pending_changes = self.has_pending_changes;

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
                    collapsed,
                    viewed,
                    filter,
                    filter_focused,
                    view_mode,
                    branch,
                    base,
                    tree_state,
                    sidebar_collapsed,
                    has_pending_changes,
                );
            }

            if mode == ViewMode::Help {
                layout::render_help(frame, area);
            }
        })
    }

    fn handle_event(&mut self, event: TerminalEvent, git: &dyn GitRepo) -> Result<()> {
        match event {
            TerminalEvent::Key(key) => self.handle_key(key.code, key.modifiers, git),
            TerminalEvent::Mouse(mouse) => self.handle_mouse(mouse),
            TerminalEvent::Resize(_, _) => Ok(()),
        }
    }

    fn handle_mouse(&mut self, event: MouseEvent) -> Result<()> {
        // Skip mouse events if in help mode
        if self.mode == ViewMode::Help {
            return Ok(());
        }

        let scroll_amount = 3; // Lines to scroll per mouse wheel tick
        let max_scroll = self.diff_lines.len().saturating_sub(1);

        match event {
            MouseEvent::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(scroll_amount);
            }
            MouseEvent::ScrollDown => {
                self.scroll = (self.scroll + scroll_amount).min(max_scroll);
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers, git: &dyn GitRepo) -> Result<()> {
        // Handle filter input mode first
        if self.focus == Focus::FilterInput {
            return self.handle_filter_key(code);
        }

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

        // 'r' to refresh
        if code == KeyCode::Char('r') {
            self.refresh(git)?;
            return Ok(());
        }

        // Tab to switch focus
        if code == KeyCode::Tab {
            self.focus = match self.focus {
                Focus::FileTree => Focus::DiffView,
                Focus::DiffView => Focus::FileTree,
                Focus::FilterInput => Focus::FileTree,
            };
            return Ok(());
        }

        // '/' to focus filter
        if code == KeyCode::Char('/') {
            self.focus = Focus::FilterInput;
            return Ok(());
        }

        // 's' to toggle split view
        if code == KeyCode::Char('s') {
            self.diff_view_mode = match self.diff_view_mode {
                diff_view::DiffViewMode::Unified => diff_view::DiffViewMode::Split,
                diff_view::DiffViewMode::Split => diff_view::DiffViewMode::Unified,
            };
            self.rebuild_diff_lines();
            return Ok(());
        }

        // 'b' to toggle sidebar
        if code == KeyCode::Char('b') {
            self.sidebar_collapsed = !self.sidebar_collapsed;
            return Ok(());
        }

        match self.focus {
            Focus::FileTree => self.handle_tree_key(code),
            Focus::DiffView => self.handle_diff_key(code),
            Focus::FilterInput => Ok(()), // Handled above
        }
    }

    fn handle_filter_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Esc | KeyCode::Enter => {
                self.focus = Focus::FileTree;
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.rebuild_flat_items();
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.rebuild_flat_items();
            }
            _ => {}
        }
        Ok(())
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
            KeyCode::Char('g') => {
                self.selected_tree_item = 0;
            }
            KeyCode::Char('G') => {
                self.selected_tree_item = max_item;
            }
            KeyCode::Enter => {
                if let Some(item) = self.flat_items.get(self.selected_tree_item) {
                    if item.is_directory {
                        // Toggle directory expansion
                        file_tree::toggle_directory(&mut self.tree_nodes, &item.tree_path);
                        self.rebuild_flat_items();
                    } else if let Some(file_idx) = item.file_index {
                        // Jump to file in diff
                        self.current_file_index = file_idx;
                        self.scroll = diff_view::find_file_start(&self.diff_lines, file_idx);
                        self.focus = Focus::DiffView;
                    }
                }
            }
            KeyCode::Char('c') => {
                // Toggle collapse for current file
                if let Some(item) = self.flat_items.get(self.selected_tree_item) {
                    if let Some(file_idx) = item.file_index {
                        if self.collapsed_files.contains(&file_idx) {
                            self.collapsed_files.remove(&file_idx);
                        } else {
                            self.collapsed_files.insert(file_idx);
                        }
                        self.rebuild_diff_lines();
                    }
                }
            }
            KeyCode::Char('v') => {
                // Toggle viewed status for current file
                if let Some(item) = self.flat_items.get(self.selected_tree_item) {
                    if let Some(file_idx) = item.file_index {
                        self.toggle_viewed(file_idx);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Toggle viewed status for a file and persist to state store.
    fn toggle_viewed(&mut self, file_idx: usize) {
        if let Some(file) = self.diff.files.get(file_idx) {
            let file_path = &file.path;

            if self.viewed_files.contains(&file_idx) {
                // Unmark as viewed
                self.viewed_files.remove(&file_idx);
                self.viewed_timestamps.remove(file_path);

                if let Some(ref store) = self.state_store {
                    let _ = store.unmark_viewed(&self.repo_path, &self.branch, file_path);
                }
            } else {
                // Mark as viewed
                self.viewed_files.insert(file_idx);
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                self.viewed_timestamps.insert(file_path.clone(), now_ms);

                if let Some(ref store) = self.state_store {
                    let _ = store.mark_viewed(&self.repo_path, &self.branch, file_path);
                }
            }
        }
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
                    self.scroll =
                        diff_view::find_file_start(&self.diff_lines, self.current_file_index);
                    self.sync_tree_selection();
                }
            }
            KeyCode::Char('p') => {
                // Previous file
                if self.current_file_index > 0 {
                    self.current_file_index -= 1;
                    self.scroll =
                        diff_view::find_file_start(&self.diff_lines, self.current_file_index);
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
            KeyCode::Char('c') => {
                // Toggle collapse for current file
                if self.collapsed_files.contains(&self.current_file_index) {
                    self.collapsed_files.remove(&self.current_file_index);
                } else {
                    self.collapsed_files.insert(self.current_file_index);
                }
                self.rebuild_diff_lines();
            }
            KeyCode::Char('v') => {
                // Toggle viewed status for current file
                self.toggle_viewed(self.current_file_index);
            }
            KeyCode::Enter => {
                // Toggle collapse on the current file when on its header
                if let Some(line) = self.diff_lines.get(self.scroll) {
                    if line.kind == diff_view::LineKind::FileHeader {
                        let file_idx = line.file_index;
                        if self.collapsed_files.contains(&file_idx) {
                            self.collapsed_files.remove(&file_idx);
                        } else {
                            self.collapsed_files.insert(file_idx);
                        }
                        self.rebuild_diff_lines();
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn sync_tree_selection(&mut self) {
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
                commits: vec![Commit {
                    hash: "def456".to_string(),
                    short_hash: "def456".to_string(),
                    message: "Add feature".to_string(),
                    author: "Test".to_string(),
                    email: "test@example.com".to_string(),
                    timestamp: 0,
                }],
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
        fn repo_path(&self) -> Result<String> {
            Ok("/fake/repo".to_string())
        }

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

        fn workdir(&self) -> Result<std::path::PathBuf> {
            Ok(std::path::PathBuf::from("/fake/repo"))
        }
    }

    #[test]
    fn test_app_initializes() {
        let git = FakeGitRepo::new();
        let app = App::new(&git, None, None, None).unwrap();
        assert_eq!(app.mode, ViewMode::Normal);
        assert!(!app.diff.files.is_empty());
    }

    #[test]
    fn test_navigate_diff() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None, None, None).unwrap();
        app.focus = Focus::DiffView;
        assert_eq!(app.scroll, 0);

        app.handle_key(KeyCode::Down, KeyModifiers::default(), &git).unwrap();
        assert_eq!(app.scroll, 1);

        app.handle_key(KeyCode::Up, KeyModifiers::default(), &git).unwrap();
        assert_eq!(app.scroll, 0);
    }

    #[test]
    fn test_q_quits() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None, None, None).unwrap();
        assert!(!app.should_quit);

        app.handle_key(KeyCode::Char('q'), KeyModifiers::default(), &git).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_toggle_split_view() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None, None, None).unwrap();
        assert_eq!(app.diff_view_mode, diff_view::DiffViewMode::Unified);

        app.handle_key(KeyCode::Char('s'), KeyModifiers::default(), &git).unwrap();
        assert_eq!(app.diff_view_mode, diff_view::DiffViewMode::Split);

        app.handle_key(KeyCode::Char('s'), KeyModifiers::default(), &git).unwrap();
        assert_eq!(app.diff_view_mode, diff_view::DiffViewMode::Unified);
    }
}

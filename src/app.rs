//! Application state machine.

use crate::domain::{BranchPreview, Comment, Diff};
use crate::ports::{
    FileWatcher, GitRepo, KeyCode, KeyModifiers, MouseEvent, NewComment, StateStore, Terminal,
    TerminalEvent,
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
    /// Visual mode for selecting lines to comment on
    Visual,
    /// Inputting a comment
    CommentInput,
}

/// Which pane/input has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    FileTree,
    #[default]
    DiffView,
    FilterInput,
}

/// Source of the diff being displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffSource {
    /// Show only committed changes (merge-base to HEAD) - GitHub PR style
    #[default]
    Committed,
    /// Show only uncommitted changes (HEAD to working tree)
    Uncommitted,
    /// Show all changes (merge-base to working tree) with uncommitted marked
    All,
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
    /// Viewport scroll position (first visible line)
    pub scroll: usize,
    /// Cursor position within the diff view (independent of scroll)
    pub cursor: usize,
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

    // Diff source mode
    pub diff_source: DiffSource,
    // Set of file paths with uncommitted changes (for orange gutter in All mode)
    pub uncommitted_files: HashSet<String>,

    // ─── Comment/annotation system ───
    /// All comments for the current repo/branch
    pub comments: Vec<Comment>,
    /// Whether to show comments inline
    pub show_comments: bool,
    /// Visual selection anchor (the line where 'V' was pressed, uses cursor position)
    pub visual_anchor: Option<usize>,
    /// Current comment input buffer
    pub comment_input: String,
    /// The file path for the current comment being created
    pub comment_file_path: Option<String>,
    /// Author name for comments (from git config or default)
    pub comment_author: String,
    /// Currently focused comment (when navigating into a comment)
    pub focused_comment: Option<i64>,
    /// Last known viewport height (updated during render)
    pub viewport_height: usize,
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
            cursor: 0,
            collapsed_files,
            viewed_files,
            filter: String::new(),
            should_quit: false,
            tree_state: ListState::default(),
            sidebar_collapsed: false,
            repo_path: repo_path.clone(),
            branch: current_branch.clone(),
            state_store: state_store.clone(),
            file_watcher,
            has_pending_changes: false,
            viewed_timestamps,
            diff_source: DiffSource::Committed,
            uncommitted_files: HashSet::new(),
            comments: Self::load_comments(&state_store, &repo_path, &current_branch),
            show_comments: true,
            visual_anchor: None,
            comment_input: String::new(),
            comment_file_path: None,
            comment_author: Self::get_git_author(git),
            focused_comment: None,
            viewport_height: 30, // Default, updated during render
        })
    }

    /// Load comments from state store.
    fn load_comments(
        state_store: &Option<Arc<dyn StateStore>>,
        repo_path: &str,
        branch: &str,
    ) -> Vec<Comment> {
        state_store
            .as_ref()
            .and_then(|store| store.get_comments(repo_path, branch).ok())
            .unwrap_or_default()
    }

    /// Get git author name for comments.
    fn get_git_author(git: &dyn GitRepo) -> String {
        // Try to get from git config, fallback to "You"
        if let Ok(path) = git.workdir() {
            if let Ok(repo) = git2::Repository::open(&path) {
                if let Ok(config) = repo.config() {
                    if let Ok(name) = config.get_string("user.name") {
                        return name;
                    }
                }
            }
        }
        "You".to_string()
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
        self.reload_diff(git)?;

        // Clear pending changes flag and watcher
        self.has_pending_changes = false;
        if let Some(ref watcher) = self.file_watcher {
            watcher.clear_changes();
        }

        Ok(())
    }

    /// Reload diff based on current diff_source mode.
    fn reload_diff(&mut self, git: &dyn GitRepo) -> Result<()> {
        let merge_base = git.merge_base(&self.preview.base_branch)?;
        let commits = git.commits_since(&merge_base)?;

        // Get the appropriate diff based on mode
        let diff = match self.diff_source {
            DiffSource::Committed => git.diff_to_base(&merge_base)?,
            DiffSource::Uncommitted => git.uncommitted_diff()?,
            DiffSource::All => git.diff_to_workdir(&merge_base)?,
        };

        // For All mode, track which files have uncommitted changes
        self.uncommitted_files = if self.diff_source == DiffSource::All {
            git.uncommitted_diff()?
                .files
                .iter()
                .map(|f| f.path.clone())
                .collect()
        } else {
            HashSet::new()
        };

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
        // Update viewport height from terminal size
        // Subtract: header (2) + status bar (2) + sticky header (1) + margin (1)
        if let Ok((_, height)) = terminal.size() {
            self.viewport_height = (height as usize).saturating_sub(6);
        }

        // Compute visual_selection first (before any mutable borrows)
        // In normal mode, show cursor as a single-line "selection"
        let visual_selection = match self.mode {
            ViewMode::Visual | ViewMode::CommentInput => self.visual_selection(),
            _ => Some((self.cursor, self.cursor)), // Show cursor line highlight
        };

        let diff = &self.diff;
        let flat_items = &self.flat_items;
        let diff_lines = &self.diff_lines;
        let selected_tree_item = self.selected_tree_item;
        let current_file_index = self.current_file_index;
        let scroll = self.scroll;
        let cursor = self.cursor;
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
        let diff_source = self.diff_source;
        let uncommitted_files = &self.uncommitted_files;
        let comments = &self.comments;
        let show_comments = self.show_comments;
        let focused_comment = self.focused_comment;
        let focus = self.focus;

        // Build draft comment for inline rendering during comment input mode
        let draft_comment = if mode == ViewMode::CommentInput {
            visual_selection.map(|(start, end)| {
                let file_path = self.comment_file_path.clone().unwrap_or_default();
                (file_path, start, end, self.comment_input.clone())
            })
        } else {
            None
        };

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
                    cursor,
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
                    diff_source,
                    uncommitted_files,
                    comments,
                    show_comments,
                    visual_selection,
                    focused_comment,
                    draft_comment.as_ref(),
                    focus,
                    mode,
                );
            }

            // Render overlays
            if mode == ViewMode::Help {
                layout::render_help(frame, area);
            }
            // Note: Visual mode and CommentInput are shown in the status bar
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
        let vh = self.viewport_height.max(5);

        match event {
            MouseEvent::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(scroll_amount);
                // Keep cursor in view - if cursor is now below viewport, move it up
                if self.cursor >= self.scroll + vh {
                    self.cursor = (self.scroll + vh).saturating_sub(1);
                }
            }
            MouseEvent::ScrollDown => {
                self.scroll = (self.scroll + scroll_amount).min(max_scroll);
                // Keep cursor in view - if cursor is now above viewport, move it down
                if self.cursor < self.scroll {
                    self.cursor = self.scroll;
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers, git: &dyn GitRepo) -> Result<()> {
        // Handle comment input mode first
        if self.mode == ViewMode::CommentInput {
            return self.handle_comment_input_key(code);
        }

        // Handle visual mode
        if self.mode == ViewMode::Visual {
            return self.handle_visual_mode_key(code);
        }

        // Handle filter input mode
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

        // 'u' to cycle diff source (Committed -> Uncommitted -> All -> Committed)
        if code == KeyCode::Char('u') {
            self.diff_source = match self.diff_source {
                DiffSource::Committed => DiffSource::Uncommitted,
                DiffSource::Uncommitted => DiffSource::All,
                DiffSource::All => DiffSource::Committed,
            };
            self.reload_diff(git)?;
            return Ok(());
        }

        // 'C' to toggle show/hide comments
        if code == KeyCode::Char('C') {
            self.show_comments = !self.show_comments;
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

        // 'V' to enter visual mode (only in diff view)
        if code == KeyCode::Char('v') && self.focus == Focus::DiffView {
            self.enter_visual_mode();
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

    // ─── Visual mode for line selection ───

    fn enter_visual_mode(&mut self) {
        self.mode = ViewMode::Visual;
        // Anchor at current cursor position
        self.visual_anchor = Some(self.cursor);
        // Determine the file path for the current line
        if let Some(line) = self.diff_lines.get(self.cursor) {
            if let Some(file) = self.diff.files.get(line.file_index) {
                self.comment_file_path = Some(file.path.clone());
            }
        }
    }

    fn exit_visual_mode(&mut self) {
        self.mode = ViewMode::Normal;
        self.visual_anchor = None;
        self.comment_file_path = None;
    }

    fn handle_visual_mode_key(&mut self, code: KeyCode) -> Result<()> {
        let max_line = self.diff_lines.len().saturating_sub(1);

        match code {
            KeyCode::Esc => {
                self.exit_visual_mode();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                // Extend selection by moving cursor down
                if self.cursor < max_line {
                    self.cursor += 1;
                    self.ensure_cursor_visible();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                // Extend selection by moving cursor up
                self.cursor = self.cursor.saturating_sub(1);
                self.ensure_cursor_visible();
            }
            KeyCode::Char('c') | KeyCode::Enter => {
                // Open comment input
                self.mode = ViewMode::CommentInput;
                self.comment_input.clear();
            }
            _ => {}
        }
        Ok(())
    }

    /// Get the visual selection range (start_line, end_line) sorted.
    pub fn visual_selection(&self) -> Option<(usize, usize)> {
        self.visual_anchor.map(|anchor| {
            let start = anchor.min(self.cursor);
            let end = anchor.max(self.cursor);
            (start, end)
        })
    }

    // ─── Comment input mode ───

    fn handle_comment_input_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Esc => {
                // Cancel comment input, return to visual mode
                self.mode = ViewMode::Visual;
                self.comment_input.clear();
            }
            KeyCode::Enter => {
                // Submit comment if not empty
                if !self.comment_input.trim().is_empty() {
                    self.submit_comment();
                }
                self.exit_visual_mode();
                self.comment_input.clear();
            }
            KeyCode::Char(c) => {
                self.comment_input.push(c);
            }
            KeyCode::Backspace => {
                self.comment_input.pop();
            }
            _ => {}
        }
        Ok(())
    }

    fn submit_comment(&mut self) {
        let Some((start_line, end_line)) = self.visual_selection() else {
            return;
        };
        let Some(file_path) = self.comment_file_path.clone() else {
            return;
        };

        let new_comment = NewComment {
            file_path,
            start_line,
            end_line,
            body: self.comment_input.trim().to_string(),
            author: self.comment_author.clone(),
        };

        // Save to state store
        if let Some(ref store) = self.state_store {
            if let Ok(id) = store.add_comment(&self.repo_path, &self.branch, new_comment.clone()) {
                // Add to local comments list
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);

                self.comments.push(Comment {
                    id,
                    file_path: new_comment.file_path,
                    start_line: new_comment.start_line,
                    end_line: new_comment.end_line,
                    body: new_comment.body,
                    author: new_comment.author,
                    created_at: now,
                    resolved: false,
                    resolved_at: None,
                    replies: vec![],
                });
            }
        }
    }

    /// Toggle resolved status of a comment at the current cursor position.
    pub fn toggle_comment_resolved(&mut self) {
        // Find a comment that covers the current cursor position
        let cursor = self.cursor;
        if let Some(comment) = self.comments.iter_mut().find(|c| {
            cursor >= c.start_line && cursor <= c.end_line
        }) {
            comment.resolved = !comment.resolved;
            if let Some(ref store) = self.state_store {
                if comment.resolved {
                    let _ = store.resolve_comment(comment.id);
                } else {
                    let _ = store.unresolve_comment(comment.id);
                }
            }
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
            KeyCode::Char('x') => {
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
        let max_line = self.diff_lines.len().saturating_sub(1);
        let file_count = self.diff.files.len();

        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_cursor_down(max_line);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_cursor_up();
            }
            KeyCode::PageDown | KeyCode::Char('d') => {
                // Page down - skip comment focus
                self.focused_comment = None;
                self.cursor = (self.cursor + 20).min(max_line);
                self.sync_from_cursor();
            }
            KeyCode::PageUp => {
                // Page up
                self.focused_comment = None;
                self.cursor = self.cursor.saturating_sub(20);
                self.sync_from_cursor();
            }
            KeyCode::Char('n') => {
                // Next file
                self.focused_comment = None;
                if self.current_file_index < file_count.saturating_sub(1) {
                    self.current_file_index += 1;
                    let file_start = diff_view::find_file_start(&self.diff_lines, self.current_file_index);
                    self.cursor = file_start;
                    self.scroll = file_start;
                    self.sync_tree_selection();
                }
            }
            KeyCode::Char('p') => {
                // Previous file
                self.focused_comment = None;
                if self.current_file_index > 0 {
                    self.current_file_index -= 1;
                    let file_start = diff_view::find_file_start(&self.diff_lines, self.current_file_index);
                    self.cursor = file_start;
                    self.scroll = file_start;
                    self.sync_tree_selection();
                }
            }
            KeyCode::Char('g') => {
                self.focused_comment = None;
                self.cursor = 0;
                self.scroll = 0;
                self.sync_from_cursor();
            }
            KeyCode::Char('G') => {
                self.focused_comment = None;
                self.cursor = max_line;
                self.scroll = max_line.saturating_sub(20);
                self.sync_from_cursor();
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
            KeyCode::Char('x') => {
                // Toggle viewed status for current file
                self.toggle_viewed(self.current_file_index);
            }
            KeyCode::Char('R') => {
                // Toggle resolved status for focused comment or comment at cursor
                if let Some(comment_id) = self.focused_comment {
                    self.toggle_comment_resolved_by_id(comment_id);
                } else {
                    self.toggle_comment_resolved();
                }
            }
            KeyCode::Enter => {
                // Toggle collapse on the current file when cursor is on its header
                if let Some(line) = self.diff_lines.get(self.cursor) {
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

    /// Move cursor down, handling comment navigation.
    fn move_cursor_down(&mut self, max_line: usize) {
        if let Some(_comment_id) = self.focused_comment {
            // Currently focused on a comment, exit it and move to next line
            self.focused_comment = None;
            if self.cursor < max_line {
                self.cursor += 1;
            }
        } else if self.show_comments {
            // Check if there's a comment ending at current cursor that we should enter
            if let Some(comment) = self.find_comment_at_cursor_end() {
                self.focused_comment = Some(comment.id);
            } else if self.cursor < max_line {
                self.cursor += 1;
            }
        } else if self.cursor < max_line {
            self.cursor += 1;
        }
        self.sync_from_cursor();
    }

    /// Move cursor up, handling comment navigation.
    fn move_cursor_up(&mut self) {
        if let Some(_comment_id) = self.focused_comment {
            // Currently focused on a comment, exit it (stay on same line)
            self.focused_comment = None;
        } else if self.cursor > 0 {
            self.cursor -= 1;
            // Check if we should enter a comment above (at previous line's end)
            if self.show_comments {
                if let Some(comment) = self.find_comment_at_cursor_end() {
                    self.focused_comment = Some(comment.id);
                }
            }
        }
        self.sync_from_cursor();
    }

    /// Find a comment that ends at the current cursor position.
    fn find_comment_at_cursor_end(&self) -> Option<&Comment> {
        let cursor = self.cursor;
        let file_path = self.diff_lines.get(cursor)
            .and_then(|line| self.diff.files.get(line.file_index))
            .map(|f| f.path.as_str());

        file_path.and_then(|path| {
            self.comments.iter().find(|c| {
                c.file_path == path && c.end_line == cursor
            })
        })
    }

    /// Sync current_file_index and tree selection from cursor position.
    fn sync_from_cursor(&mut self) {
        self.ensure_cursor_visible();

        // Update current_file_index from cursor
        if let Some(line) = self.diff_lines.get(self.cursor) {
            if self.current_file_index != line.file_index {
                self.current_file_index = line.file_index;
                self.sync_tree_selection();
            }
        }
    }

    /// Toggle resolved by comment ID.
    fn toggle_comment_resolved_by_id(&mut self, comment_id: i64) {
        if let Some(comment) = self.comments.iter_mut().find(|c| c.id == comment_id) {
            comment.resolved = !comment.resolved;
            if let Some(ref store) = self.state_store {
                if comment.resolved {
                    let _ = store.resolve_comment(comment.id);
                } else {
                    let _ = store.unresolve_comment(comment.id);
                }
            }
        }
    }

    /// Ensure the cursor is visible in the viewport with scroll margin.
    fn ensure_cursor_visible(&mut self) {
        // Use stored viewport height (updated during render)
        let base_vh = self.viewport_height.max(5);

        // Subtract lines taken by comments in the visible range
        let comment_lines = if self.show_comments {
            self.estimate_comment_lines(self.scroll, self.scroll + base_vh)
        } else {
            0
        };
        let vh = base_vh.saturating_sub(comment_lines).max(5);

        if self.cursor < self.scroll {
            // Cursor above viewport - scroll up
            self.scroll = self.cursor;
        } else if self.cursor + 2 >= self.scroll + vh {
            // Cursor at or near bottom (1 line margin) - scroll down
            self.scroll = (self.cursor + 2).saturating_sub(vh);
        }
    }

    /// Estimate how many screen lines comments take in a given diff line range.
    fn estimate_comment_lines(&self, start: usize, end: usize) -> usize {
        let mut total = 0;
        for comment in &self.comments {
            // Comment renders after its end_line
            if comment.end_line >= start && comment.end_line < end {
                // Estimate: header(1) + author(1) + body(~2-3) + bottom(1) + replies
                let body_lines = (comment.body.len() / 60).max(1); // rough wrap estimate
                let reply_lines = comment.replies.len() * 2; // ~2 lines per reply
                total += 4 + body_lines + reply_lines;
            }
        }
        total
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

        fn uncommitted_diff(&self) -> Result<Diff> {
            Ok(Diff { files: vec![] })
        }

        fn diff_to_workdir(&self, _merge_base: &str) -> Result<Diff> {
            Ok(self.diff.clone())
        }

        fn user_name(&self) -> Result<String> {
            Ok("Test User".to_string())
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
        assert_eq!(app.cursor, 0);

        app.handle_key(KeyCode::Down, KeyModifiers::default(), &git).unwrap();
        assert_eq!(app.cursor, 1);

        app.handle_key(KeyCode::Up, KeyModifiers::default(), &git).unwrap();
        assert_eq!(app.cursor, 0);
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

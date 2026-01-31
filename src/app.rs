//! Application state machine.

use crate::domain::{BranchPreview, Comment, Diff, Reply};
use crate::keymap::{Action, Context, Keymap, build_default_keymap};
use crate::ports::{
    FileWatcher, GitRepo, KeyCode, KeyModifiers, MouseEvent, NewComment, NewReply, StateStore,
    Terminal, TerminalEvent,
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
    // Files that were viewed in a previous session (may have changes since)
    pub stale_viewed_files: HashSet<usize>,

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
    /// Comment ID we're replying to (None = creating new comment)
    pub reply_to_comment_id: Option<i64>,
    /// Last known viewport height (updated during render)
    pub viewport_height: usize,
    /// Keymap for handling key bindings with context-based dispatch
    keymap: Keymap,
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

        // Load viewed files from state store
        let (viewed_files, viewed_timestamps) =
            Self::load_viewed_state(&state_store, &repo_path, &current_branch, &diff);

        // All viewed files from previous sessions are considered "stale" (may have changes)
        // For now, we don't have actual change detection, so all are stale
        let stale_viewed_files: HashSet<usize> = viewed_files.clone();

        // Auto-collapse viewed files on startup, but NOT stale ones (keep them expanded)
        // Since all viewed files are currently marked stale, we leave them expanded
        // When proper staleness detection is added, non-stale viewed files will collapse
        let collapsed_files: HashSet<usize> = viewed_files
            .iter()
            .filter(|idx| !stale_viewed_files.contains(idx))
            .copied()
            .collect();
        let diff_lines = diff_view::build_unified_lines(&diff, &collapsed_files);

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
            stale_viewed_files,
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
            reply_to_comment_id: None,
            viewport_height: 30, // Default, updated during render
            keymap: build_default_keymap(),
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
        let mut tick_count: u32 = 0;
        const COMMENT_REFRESH_INTERVAL: u32 = 40; // ~2 seconds at 50ms poll rate

        while !self.should_quit {
            // Check for file changes
            if let Some(ref watcher) = self.file_watcher {
                if watcher.has_changes() {
                    self.has_pending_changes = true;
                }
            }

            // Periodically refresh comments from database (for live updates from agents)
            tick_count += 1;
            if tick_count >= COMMENT_REFRESH_INTERVAL {
                tick_count = 0;
                self.refresh_comments();
            }

            self.draw(terminal)?;

            if let Some(event) = terminal.poll_event(Duration::from_millis(50))? {
                self.handle_event(event, git)?;
            }
        }
        Ok(())
    }

    /// Refresh comments from the database (for live updates from CLI/agents).
    fn refresh_comments(&mut self) {
        if let Some(ref store) = self.state_store {
            if let Ok(comments) = store.get_comments(&self.repo_path, &self.branch) {
                self.comments = comments;
            }
        }
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
        let stale_viewed = &self.stale_viewed_files;
        let comments = &self.comments;
        let show_comments = self.show_comments;
        let focused_comment = self.focused_comment;
        let focus = self.focus;

        // Build draft comment for inline rendering during comment input mode (new comments only)
        let draft_comment = if mode == ViewMode::CommentInput && self.reply_to_comment_id.is_none() {
            visual_selection.map(|(start, end)| {
                let file_path = self.comment_file_path.clone().unwrap_or_default();
                (file_path, start, end, self.comment_input.clone())
            })
        } else {
            None
        };

        // Build reply info for inline rendering during reply input mode
        let reply_to_id = self.reply_to_comment_id;
        let reply_input = self.comment_input.clone();

        terminal.draw(|frame| {
            let area = frame.area();

            // Build reply_info inside the closure where we need it
            let reply_info = if mode == ViewMode::CommentInput && reply_to_id.is_some() {
                reply_to_id.map(|id| (id, reply_input.as_str()))
            } else {
                None
            };

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
                    stale_viewed,
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
                    reply_info,
                    focus,
                    mode,
                );
            }

            // Render overlays
            if mode == ViewMode::Help {
                layout::render_help(frame, area, &self.keymap);
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
        let base_vh = self.viewport_height.max(5);

        // Account for comment heights when comments are visible
        let comment_lines = if self.show_comments {
            self.estimate_comment_lines_in_range(self.scroll, self.scroll + base_vh)
        } else {
            0
        };
        let vh = base_vh.saturating_sub(comment_lines).max(3);

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

    /// Build the current context stack based on app state.
    /// More specific contexts are later in the vec (higher precedence).
    fn build_contexts(&self) -> Vec<Context> {
        let mut contexts = vec![Context::Global];

        // Add focus-based context
        match self.focus {
            Focus::FileTree => contexts.push(Context::FileTree),
            Focus::DiffView => contexts.push(Context::DiffView),
            Focus::FilterInput => contexts.push(Context::FilterInput),
        }

        // Add mode-based contexts (more specific than focus)
        match self.mode {
            ViewMode::Help => contexts.push(Context::Help),
            ViewMode::Visual => contexts.push(Context::Visual),
            ViewMode::CommentInput => contexts.push(Context::CommentInput),
            ViewMode::Normal => {}
        }

        // Add comment-focused context if applicable
        if self.focused_comment.is_some() {
            contexts.push(Context::CommentFocused);
        }

        contexts
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers, git: &dyn GitRepo) -> Result<()> {
        // Handle character input specially for text input modes
        if let KeyCode::Char(c) = code {
            if !modifiers.ctrl {
                match self.mode {
                    ViewMode::CommentInput => {
                        self.comment_input.push(c);
                        return Ok(());
                    }
                    _ => {}
                }
                if self.focus == Focus::FilterInput {
                    self.filter.push(c);
                    self.rebuild_flat_items();
                    return Ok(());
                }
            }
        }

        // Look up action from keymap
        let contexts = self.build_contexts();
        let action = self.keymap.lookup(code, modifiers, &contexts);

        if let Some(action) = action {
            self.dispatch_action(action, git)?;
        }

        Ok(())
    }

    /// Execute an action.
    fn dispatch_action(&mut self, action: Action, git: &dyn GitRepo) -> Result<()> {
        let max_line = self.diff_lines.len().saturating_sub(1);
        let file_count = self.diff.files.len();

        match action {
            // === Navigation ===
            Action::MoveDown => {
                match self.focus {
                    Focus::FileTree => {
                        let max_item = self.flat_items.len().saturating_sub(1);
                        if self.selected_tree_item < max_item {
                            self.selected_tree_item += 1;
                        }
                    }
                    Focus::DiffView => self.move_cursor_down(max_line),
                    _ => {}
                }
            }
            Action::MoveUp => {
                match self.focus {
                    Focus::FileTree => {
                        self.selected_tree_item = self.selected_tree_item.saturating_sub(1);
                    }
                    Focus::DiffView => self.move_cursor_up(),
                    _ => {}
                }
            }
            Action::HalfPageDown => {
                self.focused_comment = None;
                self.cursor = (self.cursor + self.viewport_height / 2).min(max_line);
                self.sync_from_cursor();
            }
            Action::HalfPageUp => {
                self.focused_comment = None;
                self.cursor = self.cursor.saturating_sub(self.viewport_height / 2);
                self.sync_from_cursor();
            }
            Action::PageDown => {
                self.focused_comment = None;
                self.cursor = (self.cursor + 20).min(max_line);
                self.sync_from_cursor();
            }
            Action::PageUp => {
                self.focused_comment = None;
                self.cursor = self.cursor.saturating_sub(20);
                self.sync_from_cursor();
            }
            Action::GotoTop => {
                match self.focus {
                    Focus::FileTree => self.selected_tree_item = 0,
                    Focus::DiffView => {
                        self.focused_comment = None;
                        self.cursor = 0;
                        self.scroll = 0;
                        self.sync_from_cursor();
                    }
                    _ => {}
                }
            }
            Action::GotoBottom => {
                match self.focus {
                    Focus::FileTree => {
                        self.selected_tree_item = self.flat_items.len().saturating_sub(1);
                    }
                    Focus::DiffView => {
                        self.focused_comment = None;
                        self.cursor = max_line;
                        self.scroll = max_line.saturating_sub(20);
                        self.sync_from_cursor();
                    }
                    _ => {}
                }
            }
            Action::NextFile => {
                self.focused_comment = None;
                if self.current_file_index < file_count.saturating_sub(1) {
                    self.current_file_index += 1;
                    let file_start = diff_view::find_file_start(&self.diff_lines, self.current_file_index);
                    self.cursor = file_start;
                    self.scroll = file_start;
                    self.sync_tree_selection();
                }
            }
            Action::PrevFile => {
                self.focused_comment = None;
                if self.current_file_index > 0 {
                    self.current_file_index -= 1;
                    let file_start = diff_view::find_file_start(&self.diff_lines, self.current_file_index);
                    self.cursor = file_start;
                    self.scroll = file_start;
                    self.sync_tree_selection();
                }
            }

            // === Focus ===
            Action::SwitchPane => {
                self.focus = match self.focus {
                    Focus::FileTree => Focus::DiffView,
                    Focus::DiffView => Focus::FileTree,
                    Focus::FilterInput => Focus::FileTree,
                };
            }
            Action::FocusFilter => {
                self.focus = Focus::FilterInput;
            }

            // === View toggles ===
            Action::ToggleSplitView => {
                self.diff_view_mode = match self.diff_view_mode {
                    diff_view::DiffViewMode::Unified => diff_view::DiffViewMode::Split,
                    diff_view::DiffViewMode::Split => diff_view::DiffViewMode::Unified,
                };
                self.rebuild_diff_lines();
            }
            Action::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
            }
            Action::ToggleComments => {
                self.show_comments = !self.show_comments;
            }
            Action::CycleDiffSource => {
                self.diff_source = match self.diff_source {
                    DiffSource::Committed => DiffSource::Uncommitted,
                    DiffSource::Uncommitted => DiffSource::All,
                    DiffSource::All => DiffSource::Committed,
                };
                self.reload_diff(git)?;
            }

            // === File actions ===
            Action::ToggleCollapse => {
                match self.focus {
                    Focus::FileTree => {
                        if let Some(item) = self.flat_items.get(self.selected_tree_item) {
                            if item.is_directory {
                                file_tree::toggle_directory(&mut self.tree_nodes, &item.tree_path);
                                self.rebuild_flat_items();
                            } else if let Some(file_idx) = item.file_index {
                                let is_collapsing = !self.collapsed_files.contains(&file_idx);
                                if is_collapsing {
                                    self.collapsed_files.insert(file_idx);
                                } else {
                                    self.collapsed_files.remove(&file_idx);
                                }
                                self.rebuild_diff_lines();
                                self.adjust_cursor_after_collapse(file_idx, is_collapsing);
                            }
                        }
                    }
                    Focus::DiffView => {
                        // Check if on file header
                        if let Some(line) = self.diff_lines.get(self.cursor) {
                            let file_idx = if line.kind == diff_view::LineKind::FileHeader {
                                line.file_index
                            } else {
                                self.current_file_index
                            };
                            let is_collapsing = !self.collapsed_files.contains(&file_idx);
                            if is_collapsing {
                                self.collapsed_files.insert(file_idx);
                            } else {
                                self.collapsed_files.remove(&file_idx);
                            }
                            self.rebuild_diff_lines();
                            self.adjust_cursor_after_collapse(file_idx, is_collapsing);
                        }
                    }
                    _ => {}
                }
            }
            Action::ToggleViewed => {
                match self.focus {
                    Focus::FileTree => {
                        if let Some(item) = self.flat_items.get(self.selected_tree_item) {
                            if let Some(file_idx) = item.file_index {
                                self.toggle_viewed(file_idx);
                            }
                        }
                    }
                    Focus::DiffView => {
                        self.toggle_viewed(self.current_file_index);
                    }
                    _ => {}
                }
            }
            Action::SelectFile => {
                if let Some(item) = self.flat_items.get(self.selected_tree_item) {
                    if item.is_directory {
                        file_tree::toggle_directory(&mut self.tree_nodes, &item.tree_path);
                        self.rebuild_flat_items();
                    } else if let Some(file_idx) = item.file_index {
                        self.current_file_index = file_idx;
                        let file_start = diff_view::find_file_start(&self.diff_lines, file_idx);
                        self.cursor = file_start;
                        self.scroll = file_start;
                        self.focus = Focus::DiffView;
                    }
                }
            }

            // === Comment actions ===
            Action::EnterVisualMode => {
                self.enter_visual_mode();
            }
            Action::ExitVisualMode => {
                self.exit_visual_mode();
            }
            Action::StartComment => {
                self.mode = ViewMode::CommentInput;
                self.comment_input.clear();
            }
            Action::ReplyToComment => {
                if let Some(comment_id) = self.focused_comment {
                    self.start_reply(comment_id);
                }
            }
            Action::ToggleResolved => {
                if let Some(comment_id) = self.focused_comment {
                    self.toggle_comment_resolved_by_id(comment_id);
                } else {
                    self.toggle_comment_resolved();
                }
            }
            Action::DeleteComment => {
                if let Some(comment_id) = self.focused_comment {
                    self.delete_comment(comment_id);
                }
            }

            // === Input handling ===
            Action::SubmitInput => {
                match self.mode {
                    ViewMode::CommentInput => {
                        if !self.comment_input.trim().is_empty() {
                            if self.reply_to_comment_id.is_some() {
                                self.submit_reply();
                            } else {
                                self.submit_comment();
                            }
                        }
                        self.mode = ViewMode::Normal;
                        self.exit_visual_mode();
                        self.comment_input.clear();
                        self.reply_to_comment_id = None;
                    }
                    _ => {
                        // Filter input submit
                        self.focus = Focus::FileTree;
                    }
                }
            }
            Action::CancelInput => {
                match self.mode {
                    ViewMode::CommentInput => {
                        self.mode = ViewMode::Normal;
                        self.comment_input.clear();
                        self.reply_to_comment_id = None;
                        self.exit_visual_mode();
                    }
                    _ => {
                        // Filter input cancel
                        self.focus = Focus::FileTree;
                    }
                }
            }
            Action::InputBackspace => {
                self.comment_input.pop();
            }
            Action::FilterBackspace => {
                self.filter.pop();
                self.rebuild_flat_items();
            }

            // === General ===
            Action::Refresh => {
                self.refresh(git)?;
            }
            Action::ShowHelp => {
                self.mode = ViewMode::Help;
            }
            Action::DismissHelp => {
                self.mode = ViewMode::Normal;
            }
            Action::Quit => {
                self.should_quit = true;
            }

            // These are handled specially in handle_key
            Action::FilterAddChar(_) | Action::InputAddChar(_) => {}
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

    /// Get the visual selection range (start_line, end_line) sorted.
    pub fn visual_selection(&self) -> Option<(usize, usize)> {
        self.visual_anchor.map(|anchor| {
            let start = anchor.min(self.cursor);
            let end = anchor.max(self.cursor);
            (start, end)
        })
    }

    fn submit_comment(&mut self) {
        let Some((start_idx, end_idx)) = self.visual_selection() else {
            return;
        };
        let Some(file_path) = self.comment_file_path.clone() else {
            return;
        };

        // Extract actual source line numbers from the diff lines
        // Look for new_num (right side) as that's what we're commenting on
        let start_line = self.diff_lines.get(start_idx)
            .and_then(|l| l.content.new_line_num())
            .or_else(|| {
                // If start line has no line number, find first line in range that does
                (start_idx..=end_idx)
                    .find_map(|i| self.diff_lines.get(i).and_then(|l| l.content.new_line_num()))
            })
            .map(|n| n as usize)
            .unwrap_or(start_idx);

        let end_line = self.diff_lines.get(end_idx)
            .and_then(|l| l.content.new_line_num())
            .or_else(|| {
                // If end line has no line number, find last line in range that does
                (start_idx..=end_idx).rev()
                    .find_map(|i| self.diff_lines.get(i).and_then(|l| l.content.new_line_num()))
            })
            .map(|n| n as usize)
            .unwrap_or(end_idx);

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

    /// Start replying to a comment.
    fn start_reply(&mut self, comment_id: i64) {
        self.reply_to_comment_id = Some(comment_id);
        self.comment_input.clear();
        self.mode = ViewMode::CommentInput;
    }

    /// Submit a reply to a comment.
    fn submit_reply(&mut self) {
        let Some(comment_id) = self.reply_to_comment_id else {
            return;
        };

        let body = self.comment_input.trim().to_string();
        if body.is_empty() {
            return;
        }

        // Save to state store
        if let Some(ref store) = self.state_store {
            if let Ok(reply_id) = store.add_reply(NewReply {
                comment_id,
                body: body.clone(),
                author: self.comment_author.clone(),
            }) {
                // Add to local comments list
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);

                if let Some(comment) = self.comments.iter_mut().find(|c| c.id == comment_id) {
                    comment.replies.push(Reply {
                        id: reply_id,
                        comment_id,
                        body,
                        author: self.comment_author.clone(),
                        created_at: now,
                    });
                }
            }
        }

        self.reply_to_comment_id = None;
    }

    /// Toggle resolved status of a comment at the current cursor position.
    pub fn toggle_comment_resolved(&mut self) {
        // Find a comment that covers the current cursor position (using source line numbers)
        let source_line = self.diff_lines.get(self.cursor)
            .and_then(|l| l.content.new_line_num())
            .map(|n| n as usize);
        let file_path = self.diff_lines.get(self.cursor)
            .and_then(|l| self.diff.files.get(l.file_index))
            .map(|f| f.path.as_str());

        let Some(line_num) = source_line else { return };
        let Some(path) = file_path else { return };

        if let Some(comment) = self.comments.iter_mut().find(|c| {
            c.file_path == path && line_num >= c.start_line && line_num <= c.end_line
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
                // Mark as viewed and collapse the file
                self.viewed_files.insert(file_idx);
                self.collapsed_files.insert(file_idx);
                self.stale_viewed_files.remove(&file_idx); // No longer stale after viewing
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                self.viewed_timestamps.insert(file_path.clone(), now_ms);

                if let Some(ref store) = self.state_store {
                    let _ = store.mark_viewed(&self.repo_path, &self.branch, file_path);
                }

                // Rebuild diff lines since we collapsed a file
                self.rebuild_diff_lines();
                self.adjust_cursor_after_collapse(file_idx, true);
            }
        }
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

    /// Find a comment that ends at the current cursor position (using source line numbers).
    fn find_comment_at_cursor_end(&self) -> Option<&Comment> {
        let diff_line = self.diff_lines.get(self.cursor)?;
        let source_line = diff_line.content.new_line_num()? as usize;
        let file_path = self.diff.files.get(diff_line.file_index)?.path.as_str();

        self.comments.iter().find(|c| {
            c.file_path == file_path && c.end_line == source_line
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

    /// Delete a comment by ID.
    fn delete_comment(&mut self, comment_id: i64) {
        // Remove from local state
        self.comments.retain(|c| c.id != comment_id);
        // Remove from persistent storage
        if let Some(ref store) = self.state_store {
            let _ = store.delete_comment(comment_id);
        }
        // Clear focus
        self.focused_comment = None;
    }

    /// Adjust cursor position after collapsing/expanding a file.
    /// When collapsing, move cursor to the file header if it was within the file's content.
    fn adjust_cursor_after_collapse(&mut self, collapsed_file_idx: usize, is_collapsing: bool) {
        let max_line = self.diff_lines.len().saturating_sub(1);

        // Clamp cursor to valid bounds first
        if self.cursor > max_line {
            self.cursor = max_line;
        }

        // If we just collapsed a file and cursor is beyond the new bounds,
        // or if cursor was in the collapsed file's content, move to file header
        if is_collapsing {
            // Find where this file's header is in the new diff_lines
            let file_header_idx = diff_view::find_file_start(&self.diff_lines, collapsed_file_idx);

            // Check if current cursor points to a line in a different file or is past the end
            if let Some(line) = self.diff_lines.get(self.cursor) {
                // If cursor is now on a different file or an empty line after collapse,
                // move to the collapsed file's header
                if line.file_index != collapsed_file_idx {
                    // Cursor jumped to a different file, put it on the collapsed file's header
                    self.cursor = file_header_idx;
                }
            } else {
                // Cursor is out of bounds, move to file header
                self.cursor = file_header_idx;
            }
        }

        // Ensure scroll is valid
        if self.scroll > max_line {
            self.scroll = max_line;
        }

        self.ensure_cursor_visible();
        self.sync_from_cursor();
    }

    /// Ensure the cursor is visible in the viewport with scroll margin.
    fn ensure_cursor_visible(&mut self) {
        // Use stored viewport height (updated during render)
        let base_vh = self.viewport_height.max(5);

        // When comments are shown, we need to account for comment box heights
        // between scroll and cursor. Comments take up rendered space that reduces
        // how many diff lines actually fit in the viewport.
        let comment_lines = if self.show_comments {
            self.estimate_comment_lines_in_range(self.scroll, self.cursor)
        } else {
            0
        };

        // If a comment is focused, we need extra space to display the comment box
        // The comment box is rendered after the cursor line, so we need to reserve
        // space for it below the cursor
        let focused_comment_lines = if self.show_comments {
            self.focused_comment
                .and_then(|id| self.comments.iter().find(|c| c.id == id))
                .map(|c| self.estimate_comment_height(c))
                .unwrap_or(0)
        } else {
            0
        };

        // Effective viewport height is reduced by comment rendered lines
        let vh = base_vh.saturating_sub(comment_lines).max(3);

        if self.cursor < self.scroll {
            // Cursor above viewport - scroll up to keep cursor visible
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + vh {
            // Cursor below viewport - scroll down just enough to show cursor at bottom
            self.scroll = self.cursor + 1 - vh;
        }

        // If we have a focused comment, ensure there's room to display it
        // The comment is rendered after the cursor line, so we need space below
        if focused_comment_lines > 0 {
            let lines_after_cursor = vh.saturating_sub(self.cursor.saturating_sub(self.scroll) + 1);
            if lines_after_cursor < focused_comment_lines {
                // Not enough room below cursor for comment, scroll up to make room
                let extra_scroll = focused_comment_lines.saturating_sub(lines_after_cursor);
                self.scroll = self.scroll.saturating_sub(extra_scroll);
            }
        }
    }

    /// Estimate the height of a single comment box in lines.
    fn estimate_comment_height(&self, comment: &Comment) -> usize {
        // Base: header, author, empty, body placeholder, empty, hints, border = 7
        // + ~1 line per 40 chars of body text
        // + 3 lines per reply
        let body_lines = (comment.body.len() / 40).max(1);
        let reply_lines = comment.replies.len() * 3;
        7 + body_lines + reply_lines
    }

    /// Estimate the number of rendered lines that comments take up in a range of diff lines.
    fn estimate_comment_lines_in_range(&self, start_idx: usize, end_idx: usize) -> usize {
        if self.comments.is_empty() {
            return 0;
        }

        let mut total_lines = 0;

        // Iterate through diff lines in range to find comments that end there
        for idx in start_idx..=end_idx {
            if let Some(diff_line) = self.diff_lines.get(idx) {
                let source_line = diff_line.content.new_line_num().map(|n| n as usize);
                let file_path = self.diff.files.get(diff_line.file_index).map(|f| f.path.as_str());

                if let (Some(path), Some(line_num)) = (file_path, source_line) {
                    for comment in &self.comments {
                        // Only count comments that END at this line (that's when they're rendered)
                        if comment.file_path == path && comment.end_line == line_num {
                            // Estimate comment box height:
                            // 7 base lines (header, author, empty, body placeholder, empty, hints, border)
                            // + ~1 line per 40 chars of body text (rough word wrap estimate)
                            // + 3 lines per reply
                            let body_lines = (comment.body.len() / 40).max(1);
                            let reply_lines = comment.replies.len() * 3;
                            total_lines += 7 + body_lines + reply_lines;
                        }
                    }
                }
            }
        }

        total_lines
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

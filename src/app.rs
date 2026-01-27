//! Application state machine.
//! Uses trait objects for git, generics for terminal (due to dyn-compatibility).

use crate::domain::{BranchPreview, Diff};
use crate::ports::{GitRepo, KeyCode, KeyModifiers, Terminal, TerminalEvent};
use crate::ui;
use anyhow::Result;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{ListState, TableState};
use std::time::Duration;

/// Current view in the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    Commits,
    CommitDetail { commit_index: usize },
    FullDiff,
    FileDiff { file_index: usize },
    Help,
}

/// Application state.
pub struct App {
    pub preview: BranchPreview,
    pub full_diff: Diff,
    pub commit_diffs: Vec<Option<Diff>>,
    pub view: View,
    pub previous_view: Option<View>,
    pub selected_commit: usize,
    pub selected_file: usize,
    pub scroll: usize,
    pub file_scroll: usize,
    pub should_quit: bool,
    pub table_state: TableState,
    pub list_state: ListState,
}

impl App {
    pub fn new(git: &dyn GitRepo, base_override: Option<&str>) -> Result<Self> {
        let current_branch = git.current_branch()?;
        let base_branch = base_override
            .map(String::from)
            .unwrap_or_else(|| git.detect_base_branch().unwrap_or_else(|_| "main".to_string()));

        let merge_base = git.merge_base(&base_branch)?;
        let commits = git.commits_since(&merge_base)?;
        let full_diff = git.diff_to_base(&merge_base)?;

        let commit_diffs = vec![None; commits.len()];

        Ok(Self {
            preview: BranchPreview {
                current_branch,
                base_branch,
                merge_base,
                commits,
            },
            full_diff,
            commit_diffs,
            view: View::Commits,
            previous_view: None,
            selected_commit: 0,
            selected_file: 0,
            scroll: 0,
            file_scroll: 0,
            should_quit: false,
            table_state: TableState::default(),
            list_state: ListState::default(),
        })
    }

    pub fn run<T: Terminal>(&mut self, terminal: &mut T, git: &dyn GitRepo) -> Result<()> {
        while !self.should_quit {
            self.draw(terminal)?;

            if let Some(event) = terminal.poll_event(Duration::from_millis(100))? {
                self.handle_event(event, git)?;
            }
        }
        Ok(())
    }

    fn draw<T: Terminal>(&mut self, terminal: &mut T) -> Result<()> {
        let preview = &self.preview;
        let full_diff = &self.full_diff;
        let view = &self.view;
        let selected_commit = self.selected_commit;
        let selected_file = self.selected_file;
        let scroll = self.scroll;
        let file_scroll = self.file_scroll;
        let table_state = &mut self.table_state;
        let list_state = &mut self.list_state;
        let commit_diffs = &self.commit_diffs;

        terminal.draw(|frame| {
            let area = frame.area();

            match view {
                View::Commits => {
                    let chunks = Layout::default()
                        .direction(ratatui::layout::Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(1)])
                        .split(area);

                    ui::commits::render(
                        frame,
                        chunks[0],
                        &preview.commits,
                        selected_commit,
                        &preview.current_branch,
                        &preview.base_branch,
                        table_state,
                    );
                    ui::commits::render_help(frame, chunks[1]);
                }

                View::CommitDetail { commit_index } => {
                    if let Some(commit) = preview.commits.get(*commit_index) {
                        let empty_diff = Diff::default();
                        let diff = commit_diffs
                            .get(*commit_index)
                            .and_then(|d| d.as_ref())
                            .unwrap_or(&empty_diff);

                        ui::diff::render_commit_detail(
                            frame,
                            area,
                            commit,
                            diff,
                            selected_file,
                            list_state,
                        );
                    }
                }

                View::FullDiff => {
                    ui::diff::render_full_diff(
                        frame,
                        area,
                        full_diff,
                        &preview.current_branch,
                        &preview.base_branch,
                        scroll,
                        selected_file,
                    );
                }

                View::FileDiff { file_index } => {
                    if let Some(file) = full_diff.files.get(*file_index) {
                        ui::diff::render_file_diff(frame, area, file, file_scroll);
                    }
                }

                View::Help => {
                    // Render underlying view first
                    let chunks = Layout::default()
                        .direction(ratatui::layout::Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(1)])
                        .split(area);

                    ui::commits::render(
                        frame,
                        chunks[0],
                        &preview.commits,
                        selected_commit,
                        &preview.current_branch,
                        &preview.base_branch,
                        table_state,
                    );
                    ui::commits::render_help(frame, chunks[1]);

                    // Overlay help
                    ui::help::render(frame, area);
                }
            }
        })
    }

    fn handle_event(&mut self, event: TerminalEvent, git: &dyn GitRepo) -> Result<()> {
        match event {
            TerminalEvent::Key(key) => self.handle_key(key.code, key.modifiers, git),
            TerminalEvent::Resize(_, _) => Ok(()),
        }
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        git: &dyn GitRepo,
    ) -> Result<()> {
        // Global quit
        if code == KeyCode::Char('q') && !modifiers.ctrl {
            self.should_quit = true;
            return Ok(());
        }

        // Ctrl+C quit
        if code == KeyCode::Char('c') && modifiers.ctrl {
            self.should_quit = true;
            return Ok(());
        }

        // Help toggle (except when already in help)
        if code == KeyCode::Char('?') && self.view != View::Help {
            self.previous_view = Some(self.view.clone());
            self.view = View::Help;
            return Ok(());
        }

        match &self.view {
            View::Help => {
                // Any key closes help
                if let Some(prev) = self.previous_view.take() {
                    self.view = prev;
                } else {
                    self.view = View::Commits;
                }
            }

            View::Commits => match code {
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.selected_commit < self.preview.commits.len().saturating_sub(1) {
                        self.selected_commit += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected_commit = self.selected_commit.saturating_sub(1);
                }
                KeyCode::Char('g') => {
                    self.selected_commit = 0;
                }
                KeyCode::Char('G') => {
                    self.selected_commit = self.preview.commits.len().saturating_sub(1);
                }
                KeyCode::PageDown => {
                    self.selected_commit = (self.selected_commit + 10)
                        .min(self.preview.commits.len().saturating_sub(1));
                }
                KeyCode::PageUp => {
                    self.selected_commit = self.selected_commit.saturating_sub(10);
                }
                KeyCode::Enter => {
                    // Load commit diff if not cached
                    if self.commit_diffs[self.selected_commit].is_none() {
                        let commit = &self.preview.commits[self.selected_commit];
                        if let Ok(diff) = git.commit_diff(&commit.hash) {
                            self.commit_diffs[self.selected_commit] = Some(diff);
                        }
                    }
                    self.selected_file = 0;
                    self.view = View::CommitDetail {
                        commit_index: self.selected_commit,
                    };
                }
                KeyCode::Char('d') => {
                    self.scroll = 0;
                    self.selected_file = 0;
                    self.view = View::FullDiff;
                }
                _ => {}
            },

            View::CommitDetail { commit_index } => {
                let commit_index = *commit_index;
                let file_count = self
                    .commit_diffs
                    .get(commit_index)
                    .and_then(|d| d.as_ref())
                    .map(|d| d.files.len())
                    .unwrap_or(0);

                match code {
                    KeyCode::Esc => {
                        self.view = View::Commits;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.selected_file < file_count.saturating_sub(1) {
                            self.selected_file += 1;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.selected_file = self.selected_file.saturating_sub(1);
                    }
                    KeyCode::Enter => {
                        // View file diff from commit
                        self.file_scroll = 0;
                        self.view = View::FileDiff {
                            file_index: self.selected_file,
                        };
                    }
                    _ => {}
                }
            }

            View::FullDiff => match code {
                KeyCode::Esc => {
                    self.view = View::Commits;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.scroll += 1;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll = self.scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    self.scroll += 20;
                }
                KeyCode::PageUp => {
                    self.scroll = self.scroll.saturating_sub(20);
                }
                KeyCode::Char('n') => {
                    if self.selected_file < self.full_diff.files.len().saturating_sub(1) {
                        self.selected_file += 1;
                        // Jump scroll to file (approximate)
                        self.scroll = self.selected_file * 20;
                    }
                }
                KeyCode::Char('p') => {
                    self.selected_file = self.selected_file.saturating_sub(1);
                    self.scroll = self.selected_file * 20;
                }
                KeyCode::Char('g') => {
                    self.scroll = 0;
                    self.selected_file = 0;
                }
                KeyCode::Char('G') => {
                    self.selected_file = self.full_diff.files.len().saturating_sub(1);
                    self.scroll = self.selected_file * 20;
                }
                _ => {}
            },

            View::FileDiff { .. } => match code {
                KeyCode::Esc => {
                    self.view = View::CommitDetail {
                        commit_index: self.selected_commit,
                    };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.file_scroll += 1;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.file_scroll = self.file_scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    self.file_scroll += 20;
                }
                KeyCode::PageUp => {
                    self.file_scroll = self.file_scroll.saturating_sub(20);
                }
                KeyCode::Char('g') => {
                    self.file_scroll = 0;
                }
                _ => {}
            },
        }

        Ok(())
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
                    Commit {
                        hash: "ghi789".to_string(),
                        short_hash: "ghi789".to_string(),
                        message: "Fix bug".to_string(),
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
    fn test_app_initializes_with_commits_view() {
        let git = FakeGitRepo::new();
        let app = App::new(&git, None).unwrap();
        assert_eq!(app.view, View::Commits);
        assert_eq!(app.preview.commits.len(), 2);
    }

    #[test]
    fn test_navigate_down_in_commits() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();
        assert_eq!(app.selected_commit, 0);

        app.handle_key(KeyCode::Down, KeyModifiers::NONE, &git)
            .unwrap();
        assert_eq!(app.selected_commit, 1);

        // Should not go past last commit
        app.handle_key(KeyCode::Down, KeyModifiers::NONE, &git)
            .unwrap();
        assert_eq!(app.selected_commit, 1);
    }

    #[test]
    fn test_navigate_up_in_commits() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();
        app.selected_commit = 1;

        app.handle_key(KeyCode::Up, KeyModifiers::NONE, &git)
            .unwrap();
        assert_eq!(app.selected_commit, 0);

        // Should not go below 0
        app.handle_key(KeyCode::Up, KeyModifiers::NONE, &git)
            .unwrap();
        assert_eq!(app.selected_commit, 0);
    }

    #[test]
    fn test_enter_commit_detail() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();

        app.handle_key(KeyCode::Enter, KeyModifiers::NONE, &git)
            .unwrap();
        assert_eq!(app.view, View::CommitDetail { commit_index: 0 });
    }

    #[test]
    fn test_enter_full_diff() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();

        app.handle_key(KeyCode::Char('d'), KeyModifiers::NONE, &git)
            .unwrap();
        assert_eq!(app.view, View::FullDiff);
    }

    #[test]
    fn test_escape_returns_to_commits() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();
        app.view = View::FullDiff;

        app.handle_key(KeyCode::Esc, KeyModifiers::NONE, &git)
            .unwrap();
        assert_eq!(app.view, View::Commits);
    }

    #[test]
    fn test_q_quits() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();
        assert!(!app.should_quit);

        app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE, &git)
            .unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn test_help_toggle() {
        let git = FakeGitRepo::new();
        let mut app = App::new(&git, None).unwrap();

        app.handle_key(KeyCode::Char('?'), KeyModifiers::NONE, &git)
            .unwrap();
        assert_eq!(app.view, View::Help);

        // Any key closes help
        app.handle_key(KeyCode::Esc, KeyModifiers::NONE, &git)
            .unwrap();
        assert_eq!(app.view, View::Commits);
    }
}

//! Pure data types for the branch preview domain.
//! No I/O, no dependencies on external crates beyond std.

#![allow(dead_code)]

use std::fmt;

/// A git commit with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
}

impl Commit {
    pub fn summary(&self) -> &str {
        self.message.lines().next().unwrap_or(&self.message)
    }

    pub fn relative_time(&self) -> String {
        let now = chrono::Utc::now().timestamp();
        let diff = now - self.timestamp;

        if diff < 60 {
            "just now".to_string()
        } else if diff < 3600 {
            let mins = diff / 60;
            format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
        } else if diff < 86400 {
            let hours = diff / 3600;
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else if diff < 604800 {
            let days = diff / 86400;
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        } else if diff < 2592000 {
            let weeks = diff / 604800;
            format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
        } else {
            let months = diff / 2592000;
            format!("{} month{} ago", months, if months == 1 { "" } else { "s" })
        }
    }
}

/// A single line in a diff hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Context(String),
    Addition(String),
    Deletion(String),
}

impl DiffLine {
    pub fn content(&self) -> &str {
        match self {
            DiffLine::Context(s) | DiffLine::Addition(s) | DiffLine::Deletion(s) => s,
        }
    }

    pub fn prefix(&self) -> char {
        match self {
            DiffLine::Context(_) => ' ',
            DiffLine::Addition(_) => '+',
            DiffLine::Deletion(_) => '-',
        }
    }
}

/// A hunk in a diff file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

impl Hunk {
    pub fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.old_start, self.old_lines, self.new_start, self.new_lines
        )
    }
}

/// Stats for a file diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffStats {
    pub additions: usize,
    pub deletions: usize,
}

impl DiffStats {
    pub fn new(additions: usize, deletions: usize) -> Self {
        Self {
            additions,
            deletions,
        }
    }
}

impl fmt::Display for DiffStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "+{} -{}", self.additions, self.deletions)
    }
}

/// Diff for a single file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>, // For renames
    pub hunks: Vec<Hunk>,
    pub stats: DiffStats,
    pub is_binary: bool,
}

impl FileDiff {
    pub fn display_path(&self) -> &str {
        &self.path
    }
}

/// A complete diff (multiple files).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diff {
    pub files: Vec<FileDiff>,
}

impl Diff {
    pub fn total_stats(&self) -> DiffStats {
        self.files.iter().fold(DiffStats::default(), |acc, f| DiffStats {
            additions: acc.additions + f.stats.additions,
            deletions: acc.deletions + f.stats.deletions,
        })
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// Branch preview state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchPreview {
    pub current_branch: String,
    pub base_branch: String,
    pub merge_base: String,
    pub commits: Vec<Commit>,
}

impl BranchPreview {
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }
}

/// A reply to a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub id: i64,
    pub comment_id: i64,
    pub body: String,
    pub author: String,
    pub created_at: i64, // Unix timestamp in milliseconds
}

impl Reply {
    pub fn relative_time(&self) -> String {
        relative_time_from_millis(self.created_at)
    }
}

/// A review comment/annotation on a range of lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: i64,
    pub file_path: String,
    /// Starting line in the diff (0-indexed into diff view lines)
    pub start_line: usize,
    /// Ending line in the diff (inclusive)
    pub end_line: usize,
    pub body: String,
    pub author: String,
    pub created_at: i64, // Unix timestamp in milliseconds
    pub resolved: bool,
    pub resolved_at: Option<i64>,
    /// Replies to this comment
    pub replies: Vec<Reply>,
}

impl Comment {
    pub fn relative_time(&self) -> String {
        relative_time_from_millis(self.created_at)
    }

    pub fn line_range_display(&self) -> String {
        if self.start_line == self.end_line {
            format!("L{}", self.start_line + 1)
        } else {
            format!("L{}-L{}", self.start_line + 1, self.end_line + 1)
        }
    }
}

/// Helper to format relative time from millisecond timestamp.
fn relative_time_from_millis(ts: i64) -> String {
    let now = chrono::Utc::now().timestamp_millis();
    let diff = (now - ts) / 1000; // Convert to seconds

    if diff < 60 {
        "now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        format!("{}m ago", mins)
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!("{}h ago", hours)
    } else {
        let days = diff / 86400;
        format!("{}d ago", days)
    }
}

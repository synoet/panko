//! Git repository port (trait).
//! Defines the interface for git operations without coupling to any implementation.

#![allow(dead_code)]

use crate::domain::{Commit, Diff};
use anyhow::Result;

/// Port for git repository operations.
/// Implementations may use git2, shell commands, or test fakes.
pub trait GitRepo {
    /// Get the repository root path (for identification/keying state).
    fn repo_path(&self) -> Result<String>;

    /// Get the current branch name.
    fn current_branch(&self) -> Result<String>;

    /// Detect the most likely base branch (main, master, develop).
    fn detect_base_branch(&self) -> Result<String>;

    /// Find the merge-base commit between HEAD and the given base branch.
    /// This is critical for GitHub-style diffs.
    fn merge_base(&self, base: &str) -> Result<String>;

    /// Get all commits from merge-base to HEAD.
    fn commits_since(&self, merge_base_hash: &str) -> Result<Vec<Commit>>;

    /// Get the diff from merge-base to HEAD.
    /// This is the "GitHub PR diff" - only your changes, not main's changes.
    fn diff_to_base(&self, merge_base_hash: &str) -> Result<Diff>;

    /// Get the diff for a single commit.
    fn commit_diff(&self, commit_hash: &str) -> Result<Diff>;

    /// Get working directory path for file watching.
    fn workdir(&self) -> Result<std::path::PathBuf>;
}

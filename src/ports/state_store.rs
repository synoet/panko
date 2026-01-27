//! State store port (trait).
//! Defines the interface for persisting application state.

use anyhow::Result;

/// Information about when a file was viewed.
#[derive(Debug, Clone)]
pub struct ViewedFile {
    pub file_path: String,
    pub viewed_at: i64, // Unix timestamp in milliseconds
}

/// Port for persisting application state.
pub trait StateStore: Send + Sync {
    /// Mark a file as viewed at the current time.
    fn mark_viewed(&self, repo_path: &str, branch: &str, file_path: &str) -> Result<()>;

    /// Unmark a file as viewed.
    fn unmark_viewed(&self, repo_path: &str, branch: &str, file_path: &str) -> Result<()>;

    /// Get all viewed files for a repo/branch.
    fn get_viewed_files(&self, repo_path: &str, branch: &str) -> Result<Vec<ViewedFile>>;

    /// Get the viewed timestamp for a specific file, if any.
    fn get_viewed_at(&self, repo_path: &str, branch: &str, file_path: &str) -> Result<Option<i64>>;

    /// Clear all viewed files for a repo/branch.
    fn clear_viewed(&self, repo_path: &str, branch: &str) -> Result<()>;
}

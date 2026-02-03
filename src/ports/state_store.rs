//! State store port (trait).
//! Defines the interface for persisting application state.

use crate::domain::Comment;
use anyhow::Result;

/// Information about when a file was viewed.
#[derive(Debug, Clone)]
pub struct ViewedFile {
    pub file_path: String,
    pub viewed_at: i64, // Unix timestamp in milliseconds
}

/// Input for creating a new comment (without id, timestamps).
#[derive(Debug, Clone)]
pub struct NewComment {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
    pub author: String,
}

/// Input for creating a new reply (without id, timestamps).
#[derive(Debug, Clone)]
pub struct NewReply {
    pub comment_id: i64,
    pub body: String,
    pub author: String,
}

/// Port for persisting application state.
pub trait StateStore: Send + Sync {
    /// Mark a file as viewed at the current time.
    fn mark_viewed(&self, repo_path: &str, branch: &str, file_path: &str) -> Result<()>;

    /// Unmark a file as viewed.
    fn unmark_viewed(&self, repo_path: &str, branch: &str, file_path: &str) -> Result<()>;

    /// Get all viewed files for a repo/branch.
    fn get_viewed_files(&self, repo_path: &str, branch: &str) -> Result<Vec<ViewedFile>>;

    // ─── Comment methods ───

    /// Add a new comment, returns the comment ID.
    fn add_comment(&self, repo_path: &str, branch: &str, comment: NewComment) -> Result<i64>;

    /// Get all comments for a repo/branch.
    fn get_comments(&self, repo_path: &str, branch: &str) -> Result<Vec<Comment>>;


    /// Mark a comment as resolved.
    fn resolve_comment(&self, comment_id: i64) -> Result<()>;

    /// Mark a comment as unresolved.
    fn unresolve_comment(&self, comment_id: i64) -> Result<()>;

    /// Delete a comment.
    fn delete_comment(&self, comment_id: i64) -> Result<()>;


    // ─── Reply methods ───

    /// Add a reply to a comment, returns the reply ID.
    fn add_reply(&self, reply: NewReply) -> Result<i64>;

}

//! File watcher port (trait).
//! Defines the interface for watching file system changes.


/// Events from the file watcher.
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// One or more files changed.
    Changed,
}

/// Port for watching file system changes.
pub trait FileWatcher: Send {
    /// Check if there are pending changes (non-blocking).
    fn has_changes(&self) -> bool;

    /// Clear the changes flag.
    fn clear_changes(&self);
}

//! File watcher port (trait).
//! Defines the interface for watching file system changes.

use anyhow::Result;
use std::sync::mpsc::Receiver;

/// Events from the file watcher.
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// One or more files changed.
    Changed,
}

/// Port for watching file system changes.
pub trait FileWatcher: Send {
    /// Get a receiver for file events.
    /// Returns None if watcher is not running.
    fn events(&self) -> Option<Receiver<FileEvent>>;

    /// Check if there are pending changes (non-blocking).
    fn has_changes(&self) -> bool;

    /// Clear the changes flag.
    fn clear_changes(&self);

    /// Stop watching.
    fn stop(&mut self) -> Result<()>;
}

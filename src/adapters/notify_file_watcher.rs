//! Notify implementation of the FileWatcher port.

use crate::ports::{FileEvent, FileWatcher};
use anyhow::{Context, Result};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

pub struct NotifyFileWatcher {
    _watcher: RecommendedWatcher,
    event_rx: Receiver<FileEvent>,
    has_changes: Arc<AtomicBool>,
}

impl NotifyFileWatcher {
    /// Create a new file watcher for the given directory.
    pub fn new(watch_path: &Path) -> Result<Self> {
        let has_changes = Arc::new(AtomicBool::new(false));
        let has_changes_clone = has_changes.clone();

        let (event_tx, event_rx): (Sender<FileEvent>, Receiver<FileEvent>) = mpsc::channel();

        // Create watcher with debouncing
        let config = Config::default()
            .with_poll_interval(Duration::from_secs(1));

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    // Only care about modifications, creations, and deletions
                    use notify::EventKind::*;
                    match event.kind {
                        Create(_) | Modify(_) | Remove(_) => {
                            // Skip .git directory changes
                            let dominated_by_git = event.paths.iter().any(|p| {
                                p.components().any(|c| c.as_os_str() == ".git")
                            });

                            if !dominated_by_git {
                                has_changes_clone.store(true, Ordering::SeqCst);
                                let _ = event_tx.send(FileEvent::Changed);
                            }
                        }
                        _ => {}
                    }
                }
            },
            config,
        ).context("Failed to create file watcher")?;

        watcher.watch(watch_path, RecursiveMode::Recursive)
            .context("Failed to start watching directory")?;

        Ok(Self {
            _watcher: watcher,
            event_rx,
            has_changes,
        })
    }
}

impl FileWatcher for NotifyFileWatcher {
    fn has_changes(&self) -> bool {
        // Also drain any pending events to prevent channel from filling up
        while self.event_rx.try_recv().is_ok() {}
        self.has_changes.load(Ordering::SeqCst)
    }

    fn clear_changes(&self) {
        self.has_changes.store(false, Ordering::SeqCst);
        // Drain any pending events
        while self.event_rx.try_recv().is_ok() {}
    }
}

//! rev - Branch PR Preview TUI
//!
//! A TUI that previews your branch as a GitHub PR would show it,
//! using merge-base diff to show only your changes.

mod adapters;
mod app;
mod domain;
mod ports;
mod ui;

use adapters::{CrosstermTerminal, Git2Repo, NotifyFileWatcher, SqliteStateStore};
use anyhow::{Context, Result};
use clap::Parser;
use ports::GitRepo;
use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use std::io;
use std::panic;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "rev")]
#[command(about = "Preview your branch as a GitHub PR")]
#[command(version)]
struct Args {
    /// Base branch to compare against (default: auto-detect main/master)
    #[arg(short, long)]
    base: Option<String>,

    /// Path to git repository (default: current directory)
    #[arg(short, long)]
    path: Option<String>,
}

fn main() -> Result<()> {
    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal state
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        // Call original panic hook
        original_hook(panic_info);
    }));

    let args = Args::parse();

    // Open git repo
    let git = if let Some(path) = &args.path {
        Git2Repo::open(std::path::Path::new(path))
    } else {
        Git2Repo::open_current_dir()
    }
    .context("Failed to open git repository. Are you in a git directory?")?;

    // Initialize state store (SQLite for persisting viewed files)
    let state_store: Option<Arc<dyn ports::StateStore>> =
        match SqliteStateStore::new() {
            Ok(store) => Some(Arc::new(store)),
            Err(e) => {
                eprintln!("Warning: Could not initialize state store: {}. Viewed state will not persist.", e);
                None
            }
        };

    // Initialize file watcher
    let file_watcher: Option<Box<dyn ports::FileWatcher>> = match git.workdir() {
        Ok(workdir) => match NotifyFileWatcher::new(&workdir) {
            Ok(watcher) => Some(Box::new(watcher)),
            Err(e) => {
                eprintln!("Warning: Could not initialize file watcher: {}", e);
                None
            }
        },
        Err(_) => None,
    };

    // Initialize terminal
    let mut terminal = CrosstermTerminal::new().context("Failed to initialize terminal")?;

    // Create and run app
    let mut app = app::App::new(&git, args.base.as_deref(), state_store, file_watcher)
        .context("Failed to initialize app. Do you have commits ahead of the base branch?")?;

    let result = app.run(&mut terminal, &git);

    // Terminal cleanup happens in Drop

    result
}

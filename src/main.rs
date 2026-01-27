//! rev - Branch PR Preview TUI
//!
//! A TUI that previews your branch as a GitHub PR would show it,
//! using merge-base diff to show only your changes.

mod adapters;
mod app;
mod domain;
mod ports;
mod ui;

use adapters::{CrosstermTerminal, Git2Repo};
use anyhow::{Context, Result};
use clap::Parser;

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
    let args = Args::parse();

    // Open git repo
    let git = if let Some(path) = &args.path {
        Git2Repo::open(std::path::Path::new(path))
    } else {
        Git2Repo::open_current_dir()
    }
    .context("Failed to open git repository. Are you in a git directory?")?;

    // Initialize terminal
    let mut terminal = CrosstermTerminal::new().context("Failed to initialize terminal")?;

    // Create and run app
    let mut app = app::App::new(&git, args.base.as_deref())
        .context("Failed to initialize app. Do you have commits ahead of the base branch?")?;

    let result = app.run(&mut terminal, &git);

    // Terminal cleanup happens in Drop

    result
}
// test

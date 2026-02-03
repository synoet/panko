//! panko - Branch PR Preview TUI
//!
//! A TUI that previews your branch as a GitHub PR would show it,
//! using merge-base diff to show only your changes.

mod adapters;
mod app;
mod domain;
mod keymap;
mod ports;
mod ui;

use adapters::{CrosstermTerminal, Git2Repo, JjRepo, NotifyFileWatcher, SqliteStateStore};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ports::{GitRepo, StateStore};
use ui::theme;
use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use std::io;
use std::panic;
use std::sync::Arc;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(name = "panko")]
#[command(about = "Preview your branch as a GitHub PR")]
#[command(version)]
struct Args {
    /// Base branch to compare against (default: auto-detect main/master)
    #[arg(short, long, global = true)]
    base: Option<String>,

    /// Path to git repository (default: current directory)
    #[arg(short, long, global = true)]
    path: Option<String>,

    /// Theme name (e.g. github-dark, github-light)
    #[arg(long, global = true)]
    theme: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List all comments for the current branch (for AI agents)
    Comments {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Filter by status: all (default), open, resolved
        #[arg(short, long, default_value = "all")]
        status: String,
    },

    /// Resolve a comment by ID (for AI agents)
    Resolve {
        /// Comment ID to resolve
        id: i64,
    },

    /// Unresolve a comment by ID (for AI agents)
    Unresolve {
        /// Comment ID to unresolve
        id: i64,
    },

    /// Reply to a comment (for AI agents)
    Reply {
        /// Comment ID to reply to
        id: i64,

        /// Reply message
        #[arg(short, long)]
        message: String,

        /// Author name (default: git user or "Agent")
        #[arg(short, long)]
        author: Option<String>,
    },

    /// Add a new comment (for AI agents)
    Comment {
        /// File path (relative to repo root)
        file: String,

        /// Start line number (1-indexed, in the NEW version of the file)
        start: usize,

        /// End line number (1-indexed, in the NEW version of the file)
        end: usize,

        /// Comment message
        #[arg(short, long)]
        message: String,

        /// Author name (default: git user or "Agent")
        #[arg(short, long)]
        author: Option<String>,
    },

    /// Delete a comment by ID (for AI agents)
    Delete {
        /// Comment ID to delete
        id: i64,
    },

    /// Show a specific comment thread by ID (for AI agents)
    Show {
        /// Comment ID to show
        id: i64,

        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    theme::init_from_env_and_arg(args.theme.as_deref())
        .map_err(|err| anyhow::anyhow!(err))?;

    // Open repo (jj if available, else git)
    let git = open_repo(args.path.as_deref())
        .context("Failed to open repository. Are you in a git or jj directory?")?;

    // Handle subcommands (CLI mode for agents)
    if let Some(command) = args.command {
        return run_cli_command(command, git.as_ref());
    }

    // TUI mode: set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal state
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        // Call original panic hook
        original_hook(panic_info);
    }));

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
    let mut app = app::App::new(git.as_ref(), args.base.as_deref(), state_store, file_watcher)
        .context("Failed to initialize app. Do you have commits ahead of the base branch?")?;

    let result = app.run(&mut terminal, git.as_ref());

    // Terminal cleanup happens in Drop

    result
}

/// Run CLI commands (for AI agents)
fn run_cli_command(command: Command, git: &dyn GitRepo) -> Result<()> {
    let state_store = SqliteStateStore::new()
        .context("Failed to initialize state store")?;

    let repo_path = git.workdir()?.to_string_lossy().trim_end_matches('/').to_string();
    let branch = git.current_branch()?;

    match command {
        Command::Comments { format, status } => {
            let comments = state_store.get_comments(&repo_path, &branch)?;

            let filtered: Vec<_> = comments
                .iter()
                .filter(|c| match status.as_str() {
                    "open" => !c.resolved,
                    "resolved" => c.resolved,
                    _ => true, // "all"
                })
                .collect();

            if format == "json" {
                print_comments_json(&filtered);
            } else {
                print_comments_text(&filtered);
            }
        }

        Command::Resolve { id } => {
            state_store.resolve_comment(id)?;
            println!("Resolved comment #{}", id);
        }

        Command::Unresolve { id } => {
            state_store.unresolve_comment(id)?;
            println!("Unresolved comment #{}", id);
        }

        Command::Reply { id, message, author } => {
            let author = author.unwrap_or_else(|| get_git_user(git));
            let reply_id = state_store.add_reply(ports::NewReply {
                comment_id: id,
                body: message,
                author,
            })?;
            println!("Added reply #{} to comment #{}", reply_id, id);
        }

        Command::Comment { file, start, end, message, author } => {
            let author = author.unwrap_or_else(|| get_git_user(git));
            let comment_id = state_store.add_comment(&repo_path, &branch, ports::NewComment {
                file_path: file.clone(),
                start_line: start,
                end_line: end,
                body: message,
                author,
            })?;
            println!("Added comment #{} on {} lines {}-{}", comment_id, file, start, end);
        }

        Command::Delete { id } => {
            state_store.delete_comment(id)?;
            println!("Deleted comment #{}", id);
        }

        Command::Show { id, format } => {
            let comments = state_store.get_comments(&repo_path, &branch)?;
            let comment = comments.iter().find(|c| c.id == id);

            match comment {
                Some(c) => {
                    if format == "json" {
                        print_comment_json(c);
                    } else {
                        print_comment_text(c);
                    }
                }
                None => {
                    eprintln!("Comment #{} not found", id);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

fn get_git_user(git: &dyn GitRepo) -> String {
    git.user_name().unwrap_or_else(|_| "Agent".to_string())
}

fn open_repo(path: Option<&str>) -> Result<Box<dyn GitRepo>> {
    if let Some(path) = path {
        let path = Path::new(path);
        if let Ok(jj) = JjRepo::open(path) {
            return Ok(Box::new(jj));
        }
        let git = Git2Repo::open(path)?;
        return Ok(Box::new(git));
    }

    if let Ok(jj) = JjRepo::open_current_dir() {
        return Ok(Box::new(jj));
    }

    let git = Git2Repo::open_current_dir()?;
    Ok(Box::new(git))
}

fn print_comments_text(comments: &[&domain::Comment]) {
    if comments.is_empty() {
        println!("No comments found.");
        return;
    }

    for comment in comments {
        let status = if comment.resolved { "RESOLVED" } else { "OPEN" };
        let status_icon = if comment.resolved { "✓" } else { "○" };

        println!("──────────────────────────────────────");
        println!("{} #{} [{}]", status_icon, comment.id, status);
        println!("  File: {} {}", comment.file_path, comment.line_range_display());
        println!("  Author: {} ({})", comment.author, comment.relative_time());
        println!("  ");
        for line in comment.body.lines() {
            println!("  {}", line);
        }

        // Print replies
        for reply in &comment.replies {
            println!("  ");
            println!("    ↳ {} ({})", reply.author, reply.relative_time());
            for line in reply.body.lines() {
                println!("      {}", line);
            }
        }
    }
    println!("──────────────────────────────────────");
    println!("\nTotal: {} comment(s)", comments.len());
}

fn print_comments_json(comments: &[&domain::Comment]) {
    // Simple JSON output without serde
    println!("[");
    for (i, comment) in comments.iter().enumerate() {
        let replies_json: Vec<String> = comment
            .replies
            .iter()
            .map(|r| {
                format!(
                    r#"    {{"id": {}, "author": "{}", "body": "{}", "created_at": {}}}"#,
                    r.id,
                    escape_json(&r.author),
                    escape_json(&r.body),
                    r.created_at
                )
            })
            .collect();

        println!(
            r#"  {{
    "id": {},
    "file_path": "{}",
    "start_line": {},
    "end_line": {},
    "body": "{}",
    "author": "{}",
    "created_at": {},
    "resolved": {},
    "replies": [
{}
    ]
  }}{}"#,
            comment.id,
            escape_json(&comment.file_path),
            comment.start_line,
            comment.end_line,
            escape_json(&comment.body),
            escape_json(&comment.author),
            comment.created_at,
            comment.resolved,
            replies_json.join(",\n"),
            if i < comments.len() - 1 { "," } else { "" }
        );
    }
    println!("]");
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn print_comment_text(comment: &domain::Comment) {
    let status = if comment.resolved { "RESOLVED" } else { "OPEN" };
    let status_icon = if comment.resolved { "✓" } else { "○" };

    println!("──────────────────────────────────────");
    println!("{} #{} [{}]", status_icon, comment.id, status);
    println!("  File: {} {}", comment.file_path, comment.line_range_display());
    println!("  Author: {} ({})", comment.author, comment.relative_time());
    println!();
    for line in comment.body.lines() {
        println!("  {}", line);
    }

    // Print replies
    if !comment.replies.is_empty() {
        println!();
        println!("  Replies ({}):", comment.replies.len());
        for reply in &comment.replies {
            println!();
            println!("    ↳ {} ({})", reply.author, reply.relative_time());
            for line in reply.body.lines() {
                println!("      {}", line);
            }
        }
    }
    println!("──────────────────────────────────────");
}

fn print_comment_json(comment: &domain::Comment) {
    let replies_json: Vec<String> = comment
        .replies
        .iter()
        .map(|r| {
            format!(
                r#"    {{"id": {}, "author": "{}", "body": "{}", "created_at": {}}}"#,
                r.id,
                escape_json(&r.author),
                escape_json(&r.body),
                r.created_at
            )
        })
        .collect();

    println!(
        r#"{{
  "id": {},
  "file_path": "{}",
  "start_line": {},
  "end_line": {},
  "body": "{}",
  "author": "{}",
  "created_at": {},
  "resolved": {},
  "replies": [
{}
  ]
}}"#,
        comment.id,
        escape_json(&comment.file_path),
        comment.start_line,
        comment.end_line,
        escape_json(&comment.body),
        escape_json(&comment.author),
        comment.created_at,
        comment.resolved,
        replies_json.join(",\n"),
    );
}

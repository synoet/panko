//! panko - Branch PR Preview TUI
//!
//! A TUI that previews your branch as a GitHub PR would show it,
//! using merge-base diff to show only your changes.

mod adapters;
mod app;
mod domain;
mod keymap;
mod ports;
mod search;
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

#[derive(Subcommand, Debug, Clone)]
enum InitTarget {
    /// Set up Claude Code integration (.claude/skills + settings.json)
    Claude,
    /// Set up OpenAI Codex integration (AGENTS.md)
    Codex,
    /// Set up OpenCode integration (AGENTS.md)
    Opencode,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Initialize panko integration for AI coding tools
    Init {
        #[command(subcommand)]
        target: InitTarget,
    },

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
    // Handle init command separately (doesn't need branch/state)
    if let Command::Init { target } = command {
        let workdir = git.workdir()?;
        return run_init_command(target, &workdir);
    }

    let state_store = SqliteStateStore::new()
        .context("Failed to initialize state store")?;

    let repo_path = git.workdir()?.to_string_lossy().trim_end_matches('/').to_string();
    let branch = git.current_branch()?;

    match command {
        Command::Init { .. } => unreachable!(),

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

// ─── Init command ───────────────────────────────────────────────────────────

fn run_init_command(target: InitTarget, workdir: &Path) -> Result<()> {
    match target {
        InitTarget::Claude => init_claude(workdir),
        InitTarget::Codex => init_codex(workdir),
        InitTarget::Opencode => init_opencode(workdir),
    }
}

/// Panko permissions to auto-approve in Claude settings
const PANKO_PERMISSIONS: &[&str] = &[
    "Bash(panko comments*)",
    "Bash(panko show*)",
    "Bash(panko resolve*)",
    "Bash(panko unresolve*)",
    "Bash(panko reply*)",
    "Bash(panko comment*)",
    "Bash(panko delete*)",
];

fn merge_panko_permissions(settings_path: &Path) -> Result<()> {
    use std::fs;

    let content = fs::read_to_string(settings_path)
        .context("Failed to read settings file")?;
    let mut json: serde_json::Value = serde_json::from_str(&content)
        .context("Failed to parse settings JSON")?;

    // Get or create permissions.allow array
    let permissions = json
        .as_object_mut()
        .context("Settings must be a JSON object")?
        .entry("permissions")
        .or_insert_with(|| serde_json::json!({}));

    let allow = permissions
        .as_object_mut()
        .context("permissions must be an object")?
        .entry("allow")
        .or_insert_with(|| serde_json::json!([]));

    let allow_array = allow
        .as_array_mut()
        .context("permissions.allow must be an array")?;

    // Add new permissions if not already present
    for perm in PANKO_PERMISSIONS {
        let perm_val = serde_json::Value::String(perm.to_string());
        if !allow_array.contains(&perm_val) {
            allow_array.push(perm_val);
        }
    }

    fs::write(settings_path, serde_json::to_string_pretty(&json)?)
        .context("Failed to write settings file")?;

    Ok(())
}

fn init_claude(workdir: &Path) -> Result<()> {
    use std::fs;

    let claude_dir = workdir.join(".claude");
    let skills_dir = claude_dir.join("skills");

    // Create directories
    fs::create_dir_all(&skills_dir)
        .context("Failed to create .claude/skills directory")?;

    // Write skill file
    let skill_path = skills_dir.join("panko.md");
    fs::write(&skill_path, CLAUDE_SKILL_CONTENT)
        .context("Failed to write skill file")?;
    println!("Created {}", skill_path.display());

    // Write or merge settings
    let settings_path = claude_dir.join("settings.json");
    let settings_local_path = claude_dir.join("settings.local.json");

    if settings_path.exists() {
        // Merge into existing settings.json
        merge_panko_permissions(&settings_path)?;
        println!("Merged panko permissions into {}", settings_path.display());
    } else if settings_local_path.exists() {
        // Merge into existing settings.local.json
        merge_panko_permissions(&settings_local_path)?;
        println!("Merged panko permissions into {}", settings_local_path.display());
    } else {
        // Create new settings.json
        fs::write(&settings_path, CLAUDE_SETTINGS_CONTENT)
            .context("Failed to write settings file")?;
        println!("Created {}", settings_path.display());
    }

    println!("\nClaude Code integration ready. Use /panko to address review comments.");
    Ok(())
}

fn init_codex(workdir: &Path) -> Result<()> {
    init_agents_md(workdir, "Codex")
}

fn init_opencode(workdir: &Path) -> Result<()> {
    init_agents_md(workdir, "OpenCode")
}

fn init_agents_md(workdir: &Path, tool_name: &str) -> Result<()> {
    use std::fs;

    let agents_path = workdir.join("AGENTS.md");

    if agents_path.exists() {
        // Check if panko section already exists
        let content = fs::read_to_string(&agents_path)
            .context("Failed to read existing AGENTS.md")?;

        if content.contains("## panko") || content.contains("## Panko") {
            println!("AGENTS.md already contains panko instructions.");
            return Ok(());
        }

        // Append panko section
        let new_content = format!("{}\n\n{}", content.trim_end(), AGENTS_MD_SECTION);
        fs::write(&agents_path, new_content)
            .context("Failed to update AGENTS.md")?;
        println!("Added panko section to {}", agents_path.display());
    } else {
        // Create new file
        fs::write(&agents_path, AGENTS_MD_CONTENT)
            .context("Failed to write AGENTS.md")?;
        println!("Created {}", agents_path.display());
    }

    println!("\n{} integration ready.", tool_name);
    Ok(())
}

const CLAUDE_SKILL_CONTENT: &str = r#"# panko - Code Review Comments

Manages code review comments via the panko CLI. Use when the user asks to check, address, resolve, or reply to review comments on the current branch.

## Commands

```bash
panko comments                      # List all comments
panko comments --status open        # List unresolved comments
panko comments --format json        # JSON output for parsing

panko show <id>                     # Show a specific comment thread
panko resolve <id>                  # Mark comment as resolved
panko unresolve <id>                # Reopen a resolved comment
panko reply <id> --message "text"   # Reply to a comment
panko delete <id>                   # Delete a comment

panko comment <file> <start> <end> --message "text"  # Add new comment
```

## Workflow

When addressing review comments:

1. List open comments: `panko comments --status open`
2. Read and understand each comment
3. Make the code changes
4. Reply explaining what you did: `panko reply <id> --message "Fixed by..."`
5. Resolve: `panko resolve <id>`

## Notes

- Comments are scoped to repo + branch
- Line numbers refer to source file lines (new/right side of diff)
- The `--author` flag identifies the commenter (defaults to git user)
"#;

const CLAUDE_SETTINGS_CONTENT: &str = r#"{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "permissions": {
    "allow": [
      "Bash(panko comments*)",
      "Bash(panko show*)",
      "Bash(panko resolve*)",
      "Bash(panko unresolve*)",
      "Bash(panko reply*)",
      "Bash(panko comment*)",
      "Bash(panko delete*)"
    ]
  }
}
"#;

const AGENTS_MD_SECTION: &str = r#"## panko - Code Review Comments

This project uses `panko` for code review comments. Use these commands to manage review feedback:

```bash
panko comments                      # List all comments
panko comments --status open        # List unresolved comments
panko resolve <id>                  # Mark comment as resolved
panko reply <id> --message "text"   # Reply to a comment
```

When addressing review comments:
1. List open comments: `panko comments --status open`
2. Make the code changes to address each comment
3. Reply explaining what you did: `panko reply <id> --message "Fixed by..."`
4. Resolve: `panko resolve <id>`
"#;

const AGENTS_MD_CONTENT: &str = r#"# Project Instructions

## panko - Code Review Comments

This project uses `panko` for code review comments. Use these commands to manage review feedback:

```bash
panko comments                      # List all comments
panko comments --status open        # List unresolved comments
panko resolve <id>                  # Mark comment as resolved
panko reply <id> --message "text"   # Reply to a comment
```

When addressing review comments:
1. List open comments: `panko comments --status open`
2. Make the code changes to address each comment
3. Reply explaining what you did: `panko reply <id> --message "Fixed by..."`
4. Resolve: `panko resolve <id>`
"#;

//! Jujutsu (jj) implementation of the GitRepo port.

use crate::domain::{Commit, Diff, DiffLine, DiffStats, FileDiff, Hunk};
use crate::ports::GitRepo;
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct JjRepo {
    root: PathBuf,
}

impl JjRepo {
    pub fn open(path: &Path) -> Result<Self> {
        let output = Command::new("jj")
            .arg("root")
            .current_dir(path)
            .output()
            .context("Failed to execute jj")?;

        if !output.status.success() {
            return Err(anyhow!(
                "jj root failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if root.is_empty() {
            return Err(anyhow!("jj root returned empty path"));
        }

        let repo = Self {
            root: PathBuf::from(root),
        };

        // `jj root` can succeed when a stray `.jj` directory exists, but later jj
        // commands fail because the repository metadata is incomplete. Probe with a
        // read-only command so callers can fall back to git cleanly.
        repo
            .run_jj(&["status"])
            .context("jj repository probe failed")?;

        Ok(repo)
    }

    pub fn open_current_dir() -> Result<Self> {
        Self::open(Path::new("."))
    }

    fn run_jj(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("jj")
            .args(args)
            .current_dir(&self.root)
            .output()
            .context("Failed to execute jj")?;

        if !output.status.success() {
            return Err(anyhow!(
                "jj {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl GitRepo for JjRepo {
    fn repo_path(&self) -> Result<String> {
        Ok(self
            .root
            .to_str()
            .ok_or_else(|| anyhow!("Repository path is not valid UTF-8"))?
            .to_string())
    }

    fn current_branch(&self) -> Result<String> {
        // Best-effort: use branches on @ if available, fallback to "@"
        let output = self
            .run_jj(&["log", "-r", "@", "--no-graph", "--template", "{branches}\\n"])
            .unwrap_or_default();
        let name = output.trim();
        if name.is_empty() {
            Ok("@".to_string())
        } else {
            // branches may be comma-separated; pick the first
            Ok(name.split(',').next().unwrap_or(name).trim().to_string())
        }
    }

    fn detect_base_branch(&self) -> Result<String> {
        let candidates = ["main", "master", "develop", "dev"];
        let output = self.run_jj(&["branch", "list"]).unwrap_or_default();

        let mut found = std::collections::HashSet::new();
        for line in output.lines() {
            if let Some(name) = line.split_whitespace().next() {
                let name = name.trim_end_matches(':').to_string();
                if !name.is_empty() {
                    found.insert(name);
                }
            }
        }

        for candidate in candidates {
            if found.contains(candidate) {
                return Ok(candidate.to_string());
            }
        }

        Err(anyhow!(
            "Could not detect base branch. Tried: {}",
            candidates.join(", ")
        ))
    }

    fn merge_base(&self, base: &str) -> Result<String> {
        let rev = format!("merge_base(@, {})", base);
        let output = self.run_jj(&[
            "log",
            "-r",
            rev.as_str(),
            "--no-graph",
            "--template",
            "{commit_id}\\n",
        ])?;
        let hash = output.trim();
        if hash.is_empty() {
            return Err(anyhow!("Failed to resolve merge base for {}", base));
        }
        Ok(hash.to_string())
    }

    fn commits_since(&self, merge_base_hash: &str) -> Result<Vec<Commit>> {
        let rev = format!("{}..@", merge_base_hash);
        let output = self.run_jj(&[
            "log",
            "-r",
            rev.as_str(),
            "--no-graph",
            "--template",
            "{commit_id}\\t{author.name()}\\t{author.email()}\\t{author.timestamp().unix()}\\t{description}\\n",
        ]);

        let Ok(output) = output else {
            return Ok(Vec::new());
        };

        let mut commits = Vec::new();
        for line in output.lines() {
            let mut parts = line.splitn(5, '\t');
            let hash = parts.next().unwrap_or("").to_string();
            let author = parts.next().unwrap_or("Unknown").to_string();
            let email = parts.next().unwrap_or("").to_string();
            let ts_str = parts.next().unwrap_or("0");
            let message = parts.next().unwrap_or("").to_string();
            if hash.is_empty() {
                continue;
            }
            let timestamp = ts_str.parse::<i64>().unwrap_or(0);
            let short_hash = hash.chars().take(7).collect::<String>();

            commits.push(Commit {
                hash,
                short_hash,
                message,
                author,
                email,
                timestamp,
            });
        }

        Ok(commits)
    }

    fn diff_to_base(&self, merge_base_hash: &str) -> Result<Diff> {
        let rev = format!("{}..@", merge_base_hash);
        let output = self.run_jj(&["diff", "-r", rev.as_str(), "--git"])?;
        parse_unified_diff(&output)
    }

    fn commit_diff(&self, commit_hash: &str) -> Result<Diff> {
        let output = self.run_jj(&["diff", "-r", commit_hash, "--git"])?;
        parse_unified_diff(&output)
    }

    fn workdir(&self) -> Result<PathBuf> {
        Ok(self.root.clone())
    }

    fn uncommitted_diff(&self) -> Result<Diff> {
        let output = self.run_jj(&["diff", "--git"])?;
        parse_unified_diff(&output)
    }

    fn diff_to_workdir(&self, merge_base_hash: &str) -> Result<Diff> {
        let rev = format!("{}..@", merge_base_hash);
        let output = self.run_jj(&["diff", "-r", rev.as_str(), "--git"])?;
        parse_unified_diff(&output)
    }

    fn user_name(&self) -> Result<String> {
        let output = self.run_jj(&["config", "get", "user.name"])?;
        let name = output.trim();
        if name.is_empty() {
            Err(anyhow!("jj user.name not configured"))
        } else {
            Ok(name.to_string())
        }
    }
}

#[derive(Default)]
struct FileBuilder {
    path: String,
    old_path: Option<String>,
    hunks: Vec<Hunk>,
    additions: usize,
    deletions: usize,
    is_binary: bool,
}

#[derive(Default)]
struct HunkBuilder {
    old_start: u32,
    old_lines: u32,
    new_start: u32,
    new_lines: u32,
    lines: Vec<DiffLine>,
}

fn parse_unified_diff(text: &str) -> Result<Diff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileBuilder> = None;
    let mut current_hunk: Option<HunkBuilder> = None;
    let mut pending_old_path: Option<String> = None;
    let mut pending_new_path: Option<String> = None;

    let flush_current_file = |files: &mut Vec<FileDiff>,
                              file: &mut Option<FileBuilder>,
                              hunk: &mut Option<HunkBuilder>| {
        if let Some(mut f) = file.take() {
            if let Some(h) = hunk.take() {
                f.hunks.push(Hunk {
                    old_start: h.old_start,
                    old_lines: h.old_lines,
                    new_start: h.new_start,
                    new_lines: h.new_lines,
                    lines: h.lines,
                });
            }
            files.push(FileDiff {
                path: f.path,
                old_path: f.old_path,
                hunks: f.hunks,
                stats: DiffStats::new(f.additions, f.deletions),
                is_binary: f.is_binary,
            });
        }
    };

    for line in text.lines() {
        if let Some((old_path, new_path)) = parse_diff_header(line) {
            flush_current_file(&mut files, &mut current_file, &mut current_hunk);
            pending_old_path = None;
            pending_new_path = None;
            current_file = Some(FileBuilder {
                path: new_path.clone(),
                old_path: if old_path != new_path { Some(old_path) } else { None },
                hunks: Vec::new(),
                additions: 0,
                deletions: 0,
                is_binary: false,
            });
            continue;
        }

        if line.starts_with("rename from ") {
            if let Some(f) = current_file.as_mut() {
                let old = line.trim_start_matches("rename from ").trim();
                f.old_path = Some(old.to_string());
            }
            continue;
        }

        if line.starts_with("rename to ") {
            if let Some(f) = current_file.as_mut() {
                let newp = line.trim_start_matches("rename to ").trim();
                f.path = newp.to_string();
            }
            continue;
        }

        if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            if let Some(f) = current_file.as_mut() {
                f.is_binary = true;
            }
            continue;
        }

        if line.starts_with("--- ") {
            pending_old_path = parse_path_line(line, "--- ");
            continue;
        }

        if line.starts_with("+++ ") {
            pending_new_path = parse_path_line(line, "+++ ");
            if current_file.is_none() {
                if let Some(new_path) = pending_new_path.clone() {
                    let old = pending_old_path.clone().unwrap_or_else(|| new_path.clone());
                    current_file = Some(FileBuilder {
                        path: new_path.clone(),
                        old_path: if old != new_path { Some(old) } else { None },
                        hunks: Vec::new(),
                        additions: 0,
                        deletions: 0,
                        is_binary: false,
                    });
                }
            } else if let Some(f) = current_file.as_mut() {
                if let Some(new_path) = pending_new_path.clone() {
                    f.path = new_path.clone();
                }
                if let Some(old_path) = pending_old_path.clone() {
                    if old_path != f.path {
                        f.old_path = Some(old_path);
                    }
                }
            }
            continue;
        }

        if let Some((old_start, old_lines, new_start, new_lines)) = parse_hunk_header(line) {
            if current_file.is_none() {
                let path = pending_new_path
                    .clone()
                    .or_else(|| pending_old_path.clone())
                    .unwrap_or_else(|| "<unknown>".to_string());
                current_file = Some(FileBuilder {
                    path,
                    old_path: None,
                    hunks: Vec::new(),
                    additions: 0,
                    deletions: 0,
                    is_binary: false,
                });
            }
            if let Some(h) = current_hunk.take() {
                if let Some(f) = current_file.as_mut() {
                    f.hunks.push(Hunk {
                        old_start: h.old_start,
                        old_lines: h.old_lines,
                        new_start: h.new_start,
                        new_lines: h.new_lines,
                        lines: h.lines,
                    });
                }
            }
            current_hunk = Some(HunkBuilder {
                old_start,
                old_lines,
                new_start,
                new_lines,
                lines: Vec::new(),
            });
            continue;
        }

        if let Some(h) = current_hunk.as_mut() {
            if let Some(f) = current_file.as_mut() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    f.additions += 1;
                    h.lines.push(DiffLine::Addition(line[1..].to_string()));
                } else if line.starts_with('-') && !line.starts_with("---") {
                    f.deletions += 1;
                    h.lines.push(DiffLine::Deletion(line[1..].to_string()));
                } else if let Some(stripped) = line.strip_prefix(' ') {
                    h.lines.push(DiffLine::Context(stripped.to_string()));
                }
            }
        }
    }

    flush_current_file(&mut files, &mut current_file, &mut current_hunk);

    Ok(Diff { files })
}

fn parse_diff_header(line: &str) -> Option<(String, String)> {
    if !line.starts_with("diff --git ") {
        return None;
    }
    let rest = line.trim_start_matches("diff --git ").trim();
    let mut parts = rest.split_whitespace();
    let a = parts.next()?;
    let b = parts.next()?;
    Some((clean_path(a), clean_path(b)))
}

fn parse_path_line(line: &str, prefix: &str) -> Option<String> {
    let raw = line.trim_start_matches(prefix).trim();
    if raw == "/dev/null" {
        return None;
    }
    Some(clean_path(raw))
}

fn clean_path(token: &str) -> String {
    let trimmed = token.trim_matches('"');
    let trimmed = trimmed.strip_prefix("a/").or_else(|| trimmed.strip_prefix("b/")).unwrap_or(trimmed);
    trimmed.to_string()
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    let line = line.trim();
    if !line.starts_with("@@ ") {
        return None;
    }
    let header = line.trim_start_matches("@@ ").trim();
    let (ranges, _) = header.split_once(" @@").unwrap_or((header, ""));
    let mut parts = ranges.split_whitespace();
    let old_part = parts.next()?;
    let new_part = parts.next()?;
    if !old_part.starts_with('-') || !new_part.starts_with('+') {
        return None;
    }
    let (old_start, old_lines) = parse_range(&old_part[1..])?;
    let (new_start, new_lines) = parse_range(&new_part[1..])?;
    Some((old_start, old_lines, new_start, new_lines))
}

fn parse_range(input: &str) -> Option<(u32, u32)> {
    let mut iter = input.split(',');
    let start = iter.next()?.parse::<u32>().ok()?;
    let lines = match iter.next() {
        Some(count) => count.parse::<u32>().ok().unwrap_or(1),
        None => 1,
    };
    Some((start, lines))
}

#[cfg(test)]
mod tests {
    use super::JjRepo;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn has_jj() -> bool {
        Command::new("jj").arg("--version").output().is_ok()
    }

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after UNIX_EPOCH")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("panko-{}-{}-{}", prefix, std::process::id(), ts));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        dir
    }

    #[test]
    fn open_rejects_broken_jj_metadata() {
        if !has_jj() {
            return;
        }

        let dir = make_temp_dir("broken-jj");
        fs::create_dir_all(dir.join(".jj")).expect("failed to create fake .jj directory");

        let result = JjRepo::open(&dir);
        assert!(
            result.is_err(),
            "JjRepo::open should reject incomplete .jj metadata so callers can fall back to git"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_accepts_valid_jj_repo() {
        if !has_jj() {
            return;
        }

        let dir = make_temp_dir("valid-jj");
        let status = Command::new("jj")
            .args(["git", "init", "--colocate"])
            .current_dir(&dir)
            .status()
            .expect("failed to execute jj git init");
        assert!(status.success(), "jj git init --colocate should succeed");

        let result = JjRepo::open(&dir);
        assert!(result.is_ok(), "JjRepo::open should accept valid jj repos");

        let _ = fs::remove_dir_all(&dir);
    }
}

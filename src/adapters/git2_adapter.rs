//! Git2 implementation of the GitRepo port.

use crate::domain::{Commit, Diff, DiffLine, DiffStats, FileDiff, Hunk};
use crate::ports::GitRepo;
use anyhow::{anyhow, Context, Result};
use git2::{DiffOptions, Repository, Sort};
use std::path::{Path, PathBuf};

pub struct Git2Repo {
    repo: Repository,
}

impl Git2Repo {
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path).context("Failed to open git repository")?;
        Ok(Self { repo })
    }

    pub fn open_current_dir() -> Result<Self> {
        Self::open(Path::new("."))
    }

    fn branch_exists(&self, name: &str) -> bool {
        // Check local branch
        if self.repo.find_branch(name, git2::BranchType::Local).is_ok() {
            return true;
        }
        // Check remote tracking branch
        let remote_name = format!("origin/{}", name);
        self.repo
            .find_reference(&format!("refs/remotes/{}", remote_name))
            .is_ok()
    }

    fn resolve_to_commit(&self, refspec: &str) -> Result<git2::Oid> {
        // Try as branch first
        if let Ok(branch) = self.repo.find_branch(refspec, git2::BranchType::Local) {
            if let Some(target) = branch.get().target() {
                return Ok(target);
            }
        }

        // Try as remote branch
        let remote_ref = format!("refs/remotes/origin/{}", refspec);
        if let Ok(reference) = self.repo.find_reference(&remote_ref) {
            if let Some(target) = reference.target() {
                return Ok(target);
            }
        }

        // Try as commit hash or other refspec
        let obj = self
            .repo
            .revparse_single(refspec)
            .with_context(|| format!("Failed to resolve '{}'", refspec))?;
        Ok(obj.id())
    }
}

impl GitRepo for Git2Repo {
    fn repo_path(&self) -> Result<String> {
        self.repo
            .path()
            .parent() // .git dir -> repo root
            .unwrap_or(self.repo.path())
            .to_str()
            .map(String::from)
            .ok_or_else(|| anyhow!("Repository path is not valid UTF-8"))
    }

    fn current_branch(&self) -> Result<String> {
        let head = self.repo.head().context("Failed to get HEAD")?;
        if head.is_branch() {
            head.shorthand()
                .map(String::from)
                .ok_or_else(|| anyhow!("Branch name is not valid UTF-8"))
        } else {
            // Detached HEAD - return short hash
            let oid = head.target().ok_or_else(|| anyhow!("HEAD has no target"))?;
            Ok(format!("{:.7}", oid))
        }
    }

    fn detect_base_branch(&self) -> Result<String> {
        // Common base branch names in order of preference
        let candidates = ["main", "master", "develop", "dev"];

        for candidate in candidates {
            if self.branch_exists(candidate) {
                return Ok(candidate.to_string());
            }
        }

        Err(anyhow!(
            "Could not detect base branch. Tried: {}",
            candidates.join(", ")
        ))
    }

    fn merge_base(&self, base: &str) -> Result<String> {
        let head = self.repo.head()?.target().ok_or_else(|| anyhow!("No HEAD"))?;
        let base_oid = self.resolve_to_commit(base)?;

        let merge_base = self
            .repo
            .merge_base(head, base_oid)
            .with_context(|| format!("Failed to find merge-base between HEAD and {}", base))?;

        Ok(merge_base.to_string())
    }

    fn commits_since(&self, merge_base_hash: &str) -> Result<Vec<Commit>> {
        let head = self.repo.head()?.target().ok_or_else(|| anyhow!("No HEAD"))?;
        let merge_base = git2::Oid::from_str(merge_base_hash)?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        revwalk.push(head)?;
        revwalk.hide(merge_base)?;

        let mut commits = Vec::new();
        for oid in revwalk {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            let author = commit.author();

            commits.push(Commit {
                hash: oid.to_string(),
                short_hash: format!("{:.7}", oid),
                message: commit.message().unwrap_or("").to_string(),
                author: author.name().unwrap_or("Unknown").to_string(),
                email: author.email().unwrap_or("").to_string(),
                timestamp: commit.time().seconds(),
            });
        }

        Ok(commits)
    }

    fn diff_to_base(&self, merge_base_hash: &str) -> Result<Diff> {
        let merge_base_oid = git2::Oid::from_str(merge_base_hash)?;
        let merge_base_commit = self.repo.find_commit(merge_base_oid)?;
        let merge_base_tree = merge_base_commit.tree()?;

        let head = self.repo.head()?.peel_to_commit()?;
        let head_tree = head.tree()?;

        let mut opts = DiffOptions::new();
        opts.context_lines(3);

        let diff = self
            .repo
            .diff_tree_to_tree(Some(&merge_base_tree), Some(&head_tree), Some(&mut opts))?;

        parse_git2_diff(&diff)
    }

    fn commit_diff(&self, commit_hash: &str) -> Result<Diff> {
        let oid = git2::Oid::from_str(commit_hash)?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;

        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };

        let mut opts = DiffOptions::new();
        opts.context_lines(3);

        let diff = self
            .repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;

        parse_git2_diff(&diff)
    }

    fn workdir(&self) -> Result<PathBuf> {
        self.repo
            .workdir()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("Repository has no working directory (bare repo?)"))
    }
}

fn parse_git2_diff(diff: &git2::Diff) -> Result<Diff> {
    let mut files = Vec::new();

    for delta_idx in 0..diff.deltas().len() {
        let delta = diff.get_delta(delta_idx).unwrap();
        let new_file = delta.new_file();
        let old_file = delta.old_file();

        let path = new_file
            .path()
            .or_else(|| old_file.path())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<unknown>".to_string());

        let old_path = if old_file.path() != new_file.path() {
            old_file.path().map(|p| p.to_string_lossy().into_owned())
        } else {
            None
        };

        let is_binary = delta.flags().is_binary();

        let mut hunks = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        if !is_binary {
            let patch = git2::Patch::from_diff(diff, delta_idx)?;
            if let Some(patch) = patch {
                for hunk_idx in 0..patch.num_hunks() {
                    let (hunk, _) = patch.hunk(hunk_idx)?;
                    let mut lines = Vec::new();

                    for line_idx in 0..patch.num_lines_in_hunk(hunk_idx)? {
                        let line = patch.line_in_hunk(hunk_idx, line_idx)?;
                        let content = String::from_utf8_lossy(line.content()).into_owned();
                        // Remove trailing newline for display
                        let content = content.trim_end_matches('\n').to_string();

                        match line.origin() {
                            '+' => {
                                additions += 1;
                                lines.push(DiffLine::Addition(content));
                            }
                            '-' => {
                                deletions += 1;
                                lines.push(DiffLine::Deletion(content));
                            }
                            ' ' => {
                                lines.push(DiffLine::Context(content));
                            }
                            _ => {}
                        }
                    }

                    hunks.push(Hunk {
                        old_start: hunk.old_start(),
                        old_lines: hunk.old_lines(),
                        new_start: hunk.new_start(),
                        new_lines: hunk.new_lines(),
                        lines,
                    });
                }
            }
        }

        files.push(FileDiff {
            path,
            old_path,
            hunks,
            stats: DiffStats::new(additions, deletions),
            is_binary,
        });
    }

    Ok(Diff { files })
}

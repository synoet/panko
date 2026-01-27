//! Pure business logic for branch previews.
//! No I/O - all functions are data in, data out.

use super::types::{BranchPreview, Commit, Diff, DiffStats};

/// Filter commits by search term (case-insensitive).
pub fn filter_commits<'a>(commits: &'a [Commit], search: &str) -> Vec<&'a Commit> {
    if search.is_empty() {
        return commits.iter().collect();
    }
    let search_lower = search.to_lowercase();
    commits
        .iter()
        .filter(|c| {
            c.message.to_lowercase().contains(&search_lower)
                || c.author.to_lowercase().contains(&search_lower)
                || c.short_hash.to_lowercase().contains(&search_lower)
        })
        .collect()
}

/// Compute summary statistics for a branch preview.
pub fn compute_summary(preview: &BranchPreview, diff: &Diff) -> BranchSummary {
    let stats = diff.total_stats();
    BranchSummary {
        commit_count: preview.commits.len(),
        file_count: diff.file_count(),
        additions: stats.additions,
        deletions: stats.deletions,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchSummary {
    pub commit_count: usize,
    pub file_count: usize,
    pub additions: usize,
    pub deletions: usize,
}

/// Get files sorted by most changes (additions + deletions).
pub fn files_by_churn(diff: &Diff) -> Vec<(&str, DiffStats)> {
    let mut files: Vec<_> = diff
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.stats))
        .collect();
    files.sort_by(|a, b| {
        let churn_a = a.1.additions + a.1.deletions;
        let churn_b = b.1.additions + b.1.deletions;
        churn_b.cmp(&churn_a)
    });
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_commit(hash: &str, message: &str, author: &str) -> Commit {
        Commit {
            hash: hash.to_string(),
            short_hash: hash[..7].to_string(),
            message: message.to_string(),
            author: author.to_string(),
            email: format!("{}@example.com", author.to_lowercase()),
            timestamp: 0,
        }
    }

    #[test]
    fn filter_commits_empty_search_returns_all() {
        let commits = vec![
            make_commit("abc1234567", "Fix bug", "Alice"),
            make_commit("def5678901", "Add feature", "Bob"),
        ];
        let filtered = filter_commits(&commits, "");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_commits_by_message() {
        let commits = vec![
            make_commit("abc1234567", "Fix authentication bug", "Alice"),
            make_commit("def5678901", "Add feature", "Bob"),
        ];
        let filtered = filter_commits(&commits, "auth");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].short_hash, "abc1234");
    }

    #[test]
    fn filter_commits_by_author() {
        let commits = vec![
            make_commit("abc1234567", "Fix bug", "Alice"),
            make_commit("def5678901", "Add feature", "Bob"),
        ];
        let filtered = filter_commits(&commits, "bob");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].author, "Bob");
    }

    #[test]
    fn filter_commits_case_insensitive() {
        let commits = vec![make_commit("abc1234567", "Fix BUG", "Alice")];
        let filtered = filter_commits(&commits, "bug");
        assert_eq!(filtered.len(), 1);
    }
}

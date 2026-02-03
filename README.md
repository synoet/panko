# panko

tui for reviewing local branch changes as GitHub-style pull requests.

## What it does

Panko shows your branch diff against the base branch (main/master) exactly how GitHub would display it in a PR. You can add inline comments, reply to them, and mark them resolved—all persisted locally.

It also exposes a CLI so AI coding agents can read, respond to, and resolve comments programmatically.

## CLI

```bash
panko                       # open TUI for current branch
panko --base develop        # diff against specific branch
panko --uncommitted         # show only unstaged changes

panko comments              # list comments (--json for structured output)
panko comment src/main.rs 10 15 -m "needs error handling"
panko reply <id> -m "fixed"
panko resolve <id>

panko init claude           # generate CLAUDE.md instructions for agent integration
```

# panko - Code Review Comments

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

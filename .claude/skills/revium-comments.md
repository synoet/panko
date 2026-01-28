# revium - Code Review Comments

Use this skill when the user asks you to work with code review comments in a `revium` PR preview session. This skill allows you to list, resolve, and reply to comments that reviewers (human or AI) have left on the branch.

## When to Use

- User asks you to "check for review comments", "see what comments need addressing", or similar
- User asks you to "resolve a comment" or "mark comment as done"
- User wants you to "reply to a comment" with an explanation
- User asks for a summary of open review items

## Commands

### List all comments
```bash
revium comments
```

### List only open (unresolved) comments
```bash
revium comments --status open
```

### List resolved comments
```bash
revium comments --status resolved
```

### Get JSON output (for parsing)
```bash
revium comments --format json
```

### Resolve a comment by ID
```bash
revium resolve <comment_id>
```

### Unresolve a comment (reopen)
```bash
revium unresolve <comment_id>
```

### Reply to a comment
```bash
revium reply <comment_id> --message "Your reply here"
revium reply <comment_id> --message "Your reply" --author "Claude"  # specify author
```

### Add a new comment
```bash
revium comment <file_path> <start_line> <end_line> --message "Your comment"
revium comment src/app.rs 100 105 --message "Consider error handling here" --author "Claude"
```

### Delete a comment
```bash
revium delete <comment_id>
```

### Show a specific comment thread
```bash
revium show <comment_id>
revium show <comment_id> --format json
```

## Workflow

When the user asks you to address review comments:

1. **List open comments**: `revium comments --status open`
2. **Read each comment** to understand what needs to be done
3. **Make the code changes** to address each comment
4. **Reply to the comment** explaining what you did: `revium reply <id> --message "Fixed by..."`
5. **Resolve the comment**: `revium resolve <id>`
6. **Verify** by listing open comments again

## Output Format

### Text output (default)
```
──────────────────────────────────────
○ #1 [OPEN]
  File: src/main.rs L45-L47
  Author: reviewer (2h ago)

  This function should handle the error case more gracefully.

    ↳ you (1h ago)
      I'll add proper error handling with Result types.
──────────────────────────────────────
```

### JSON output
```json
[
  {
    "id": 1,
    "file_path": "src/main.rs",
    "start_line": 44,
    "end_line": 46,
    "body": "This function should handle the error case more gracefully.",
    "author": "reviewer",
    "created_at": 1706359200000,
    "resolved": false,
    "replies": [
      {
        "id": 1,
        "author": "you",
        "body": "I'll add proper error handling with Result types.",
        "created_at": 1706362800000
      }
    ]
  }
]
```

## Example Session

```
User: Check if there are any review comments I need to address

AI: Let me check for open review comments.
[runs: revium comments --status open]

Found 2 open comments:

1. #3 on src/auth.rs L12-L15: "Add rate limiting to prevent brute force attacks"
2. #5 on src/api/users.rs L45: "This SQL query might be vulnerable to injection"

Would you like me to address these?

User: Yes, fix them both

AI: I'll address each comment:

**Comment #3 - Rate limiting**
[makes changes to src/auth.rs]
[runs: revium reply 3 --message "Added rate limiting using a token bucket algorithm with 5 attempts per minute"]
[runs: revium resolve 3]

**Comment #5 - SQL injection**
[makes changes to src/api/users.rs]
[runs: revium reply 5 --message "Fixed by using parameterized queries instead of string interpolation"]
[runs: revium resolve 5]

Done! Both comments have been addressed and resolved.
```

## Notes

- Comments are stored locally in SQLite at `~/.config/revium/state.db` (macOS: `~/Library/Application Support/revium/`)
- Comments are scoped to repo + branch, so switching branches shows different comments
- Line numbers refer to actual source file lines (in the new/right side of the diff)
- The `--author` flag lets agents identify themselves (defaults to git user name)
- Comments live-update in the TUI every ~2 seconds, so CLI changes appear automatically

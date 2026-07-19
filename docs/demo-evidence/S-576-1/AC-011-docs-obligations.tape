Output AC-011-docs-obligations.gif
Output AC-011-docs-obligations.webm

Set FontSize 14
Set Width 1400
Set Height 490
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Env PATH "/opt/homebrew/bin:/Users/zious/.cargo/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "cd /tmp/jr-s576-worktree"
Enter
Sleep 200ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-011: README row present for attachment list
Type "grep -n 'attachment list' README.md | head -3"
Enter
Sleep 400ms
Sleep 1000ms

# AC-011: CHANGELOG entry present
Type "grep -n 'attachment list' CHANGELOG.md | head -3"
Enter
Sleep 400ms
Sleep 1000ms

# AC-011: CLAUDE.md citations test passes (attachments.rs entries resolve to real files)
Type "cargo test --test claude_md_citations 2>&1 | tail -5"
Enter
Sleep 5000ms
Sleep 1500ms

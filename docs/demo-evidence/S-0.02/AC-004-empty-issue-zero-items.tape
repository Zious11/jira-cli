# AC-004: Empty issue returns zero items (BC-X.5.002 loop-termination guard)
# For an empty issue (total=0), list_worklogs returns zero items with no HTTP errors.
# Ensures the pagination loop handles the empty case cleanly without infinite looping.

Output docs/demo-evidence/S-0.02/AC-004-empty-issue-zero-items.gif
Output docs/demo-evidence/S-0.02/AC-004-empty-issue-zero-items.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1000
Set Height 400
Set FontFamily "Menlo"
Set Theme "Dracula"
Set Padding 24
Set TypingSpeed 40ms
Set PlaybackSpeed 1

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.02-paginate-list-worklogs"
Enter
Sleep 500ms
Show

Type "# AC-004: Empty issue returns zero items — loop-termination guard"
Enter
Sleep 500ms

Type "cargo test --test worklog_commands test_bc_x_5_002_empty_issue_returns_zero_items -- --nocapture --test-threads=1"
Enter
Sleep 30s

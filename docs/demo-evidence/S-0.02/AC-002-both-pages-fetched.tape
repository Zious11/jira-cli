# AC-002: Both pages fetched from server (BC-X.5.002)
# Wiremock mock with expect(2) for the worklog endpoint.
# Verifies that list_worklogs issues exactly two HTTP requests for an 80-item issue.

Output docs/demo-evidence/S-0.02/AC-002-both-pages-fetched.gif
Output docs/demo-evidence/S-0.02/AC-002-both-pages-fetched.webm

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

Type "# AC-002: Both pages fetched — wiremock expect(2) satisfied"
Enter
Sleep 500ms

Type "cargo test --test worklog_commands test_bc_x_5_002_both_pages_fetched -- --nocapture --test-threads=1"
Enter
Sleep 30s

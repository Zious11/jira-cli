# AC-001: Two-page result returns all 80 items (BC-X.5.002)
# Given wiremock returns page 1 (50 worklogs, total=80) and page 2 (30 worklogs, total=80),
# list_worklogs must return all 80 items — no silent truncation.

Output docs/demo-evidence/S-0.02/AC-001-two-page-result.gif
Output docs/demo-evidence/S-0.02/AC-001-two-page-result.webm

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

Type "# AC-001: Two-page result returns all 80 items — list_worklogs paginates correctly"
Enter
Sleep 500ms

Type "cargo test --test worklog_commands test_bc_x_5_002_two_page_result_returns_all_80_items -- --nocapture --test-threads=1"
Enter
Sleep 30s

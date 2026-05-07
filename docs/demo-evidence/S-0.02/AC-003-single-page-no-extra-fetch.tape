# AC-003: Single-page result makes only one HTTP request (BC-X.5.002 regression guard)
# For a single-page result (total=20, one page), only one HTTP request is made.
# Ensures the pagination loop terminates immediately when all items fit in one page.

Output docs/demo-evidence/S-0.02/AC-003-single-page-no-extra-fetch.gif
Output docs/demo-evidence/S-0.02/AC-003-single-page-no-extra-fetch.webm

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

Type "# AC-003: Single-page no extra fetch — regression guard for loop termination"
Enter
Sleep 500ms

Type "cargo test --test worklog_commands test_bc_x_5_002_single_page_no_extra_fetch -- --nocapture --test-threads=1"
Enter
Sleep 30s

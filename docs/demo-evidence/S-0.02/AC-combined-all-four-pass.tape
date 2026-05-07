# Combined: All four S-0.02 ACs pass in a single worklog_commands run
# Runs the complete worklog_commands integration test suite (9 tests: 4 new + 5 existing).
# Demonstrates the full worklog_commands test suite is green after the pagination fix.

Output docs/demo-evidence/S-0.02/AC-combined-all-four-pass.gif
Output docs/demo-evidence/S-0.02/AC-combined-all-four-pass.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1000
Set Height 600
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

Type "# Combined: All 4 S-0.02 ACs pass — full worklog_commands suite green"
Enter
Sleep 500ms

Type "cargo test --test worklog_commands -- --nocapture --test-threads=1"
Enter
Sleep 60s

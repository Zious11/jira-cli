# COMBINED: Full S-2.01 holdout suite — 7/7 tests green
# Covers all: H-030, H-031, H-032, H-033, H-034, H-035, H-021
# AC-001 through AC-007
#
# Output: COMBINED-all-7-tests-green.gif + .webm

Output docs/demo-evidence/S-2.01/COMBINED-all-7-tests-green.gif
Output docs/demo-evidence/S-2.01/COMBINED-all-7-tests-green.webm

Require cargo

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 30
Set TypingSpeed 40ms

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-2.01-bc-2-issue-read-holdout-suite"
Enter
Sleep 500ms
Show

Type "# S-2.01 full suite: 7 holdouts covering H-021, H-030..H-035"
Enter
Sleep 300ms

Type "cargo test --test issue_read_holdouts -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 7/7 passed — all issue-read holdout behaviors pinned"
Enter
Sleep 2s

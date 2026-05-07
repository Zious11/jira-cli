# Combined: all 6 SD-003 tests pass (6/6 green)
# Shows the full verbose_bodies test suite running to completion.
#
# Expected: 6 passed; 0 failed

Output docs/demo-evidence/S-0.06/AC-combined-all-sd-003-pass.gif
Output docs/demo-evidence/S-0.06/AC-combined-all-sd-003-pass.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.06-verbose-bodies-flag-and-pii-warning"
Enter
Sleep 500ms
Show

Type "# S-0.06 SD-003 gate: all 6 tests green"
Enter
Sleep 300ms

Type "cargo test --test verbose_bodies -- --nocapture --test-threads=1"
Enter
Sleep 120s

Type "# Result: 6 passed; 0 failed"
Enter
Sleep 2s

Output docs/demo-evidence/S-3.03/AC-Bonus-2-all-tests-and-no-regression.gif
Output docs/demo-evidence/S-3.03/AC-Bonus-2-all-tests-and-no-regression.webm

Require cargo

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# Combined: 8/8 integration tests pass + 612/612 lib tests no regression.
# Integration suite: AC-001, AC-003..AC-008 (8 always-run, 4 ignored).
# Lib suite: all existing unit tests pass.

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'cargo test --test oauth_refresh_integration 2>&1 | tail -5'
Enter
Sleep 11s
Type 'cargo test --lib 2>&1 | tail -5'
Enter
Sleep 12s

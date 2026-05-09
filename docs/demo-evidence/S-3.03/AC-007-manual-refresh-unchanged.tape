Output docs/demo-evidence/S-3.03/AC-007-manual-refresh-unchanged.gif
Output docs/demo-evidence/S-3.03/AC-007-manual-refresh-unchanged.webm

Require cargo

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# AC-007: Manual 'jr auth refresh' CLI command unchanged.
# Regression pin — passes both pre and post implementation.
# Verifies auth holdout suite (H-001..H-008, H-022, H-029) has no regressions.

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'cargo test --test oauth_refresh_integration test_manual_jr_auth_refresh_unchanged -- --nocapture 2>&1 | tail -10'
Enter
Sleep 12s

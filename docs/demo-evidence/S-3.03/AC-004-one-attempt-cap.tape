Output docs/demo-evidence/S-3.03/AC-004-one-attempt-cap.gif
Output docs/demo-evidence/S-3.03/AC-004-one-attempt-cap.webm

Require cargo

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# AC-004: Auto-refresh capped at exactly 1 attempt per request.
# Variant 1: retry also 401 -> NotAuthenticated, no second refresh.
# Variant 2: refresh fails -> NotAuthenticated, no retry.

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'cargo test --test oauth_refresh_integration test_send_caps_refresh_at_one_attempt -- --nocapture 2>&1 | tail -15'
Enter
Sleep 12s

# AC-003 (H-004): auth refresh --no-input no URL → exit 64 + actionable hints
# test_s_1_06_h_004_auth_refresh_no_url_configured
#
# Pins: auth refresh --no-input on a profile with no URL must:
#   - exit 64 (UserError)
#   - include "no URL configured" in stderr
#   - include "jr auth login" recovery command in stderr
#   - include "--url" recovery flag in stderr
#   - not panic
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-1.06/AC-003-auth-refresh-no-url.gif
Output docs/demo-evidence/S-1.06/AC-003-auth-refresh-no-url.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.06-oauth-flow-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-003: auth refresh --no-input no URL → exit 64 + actionable hints"
Enter
Sleep 300ms

Type "cargo test --test oauth_flow_holdouts test_s_1_06_h_004 -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 1 passed; 0 failed — exit 64 + recovery hints green"
Enter
Sleep 2s

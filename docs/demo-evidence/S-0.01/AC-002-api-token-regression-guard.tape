# AC-002: api-token regression guard (BC-3.4.001)
# api-token auth mode must still produce the correct instance URL.
# Given JR_BASE_URL=https://mycompany.atlassian.net, jr issue open PROJ-123 --url-only
# must emit https://mycompany.atlassian.net/browse/PROJ-123 and must NOT contain api.atlassian.com.

Output docs/demo-evidence/S-0.01/AC-002-api-token-regression-guard.gif
Output docs/demo-evidence/S-0.01/AC-002-api-token-regression-guard.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.01-fix-handle-open-oauth-instance-url"
Enter
Sleep 500ms
Show

Type "# AC-002: api-token regression guard — instance URL unchanged by OAuth fix"
Enter
Sleep 500ms

Type "cargo test --test issue_open test_bc_3_4_001_api_token_regression_guard -- --nocapture --test-threads=1"
Enter
Sleep 30s

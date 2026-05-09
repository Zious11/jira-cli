Output docs/demo-evidence/S-3.03/AC-001-auto-refresh-on-401.gif
Output docs/demo-evidence/S-3.03/AC-001-auto-refresh-on-401.webm

Require cargo

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# AC-001: JiraClient::send() auto-refreshes on 401 and retries once
# Wiremock returns 401 on first request, 200 on second.
# Caller sees success — no explicit retry needed at call sites.

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'cargo test --test oauth_refresh_integration test_send_retries_once_after_refresh_on_401 -- --nocapture 2>&1 | tail -15'
Enter
Sleep 12s

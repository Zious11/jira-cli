Output docs/demo-evidence/S-3.03/AC-003-invalid-grant-not-authenticated.gif
Output docs/demo-evidence/S-3.03/AC-003-invalid-grant-not-authenticated.webm

Require cargo

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# AC-003: invalid_grant surfaces NotAuthenticated with hint
# Wiremock returns {"error":"invalid_grant"} on POST /oauth/token.
# send() surfaces JrError::NotAuthenticated with hint "run 'jr auth refresh' to re-authenticate".

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'cargo test --test oauth_refresh_integration test_invalid_grant_surfaces_not_authenticated_with_refresh_hint -- --nocapture 2>&1 | tail -10'
Enter
Sleep 12s

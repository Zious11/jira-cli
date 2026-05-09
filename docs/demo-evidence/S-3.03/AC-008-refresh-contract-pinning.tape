Output docs/demo-evidence/S-3.03/AC-008-refresh-contract-pinning.gif
Output docs/demo-evidence/S-3.03/AC-008-refresh-contract-pinning.webm

Require cargo

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# AC-008: Wiremock contract pins 4 invariants in one test:
# (a) URL = https://auth.atlassian.com/oauth/token
# (b) grant_type=refresh_token in request body
# (c) token rotation persisted via store_oauth_tokens
# (d) invalid_grant on 4xx -> NotAuthenticated

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'cargo test --test oauth_refresh_integration test_refresh_contract_pins_url_grant_type_rotation_invalid_grant -- --nocapture 2>&1 | tail -10'
Enter
Sleep 12s

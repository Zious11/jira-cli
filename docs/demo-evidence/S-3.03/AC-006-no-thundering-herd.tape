Output docs/demo-evidence/S-3.03/AC-006-no-thundering-herd.gif
Output docs/demo-evidence/S-3.03/AC-006-no-thundering-herd.webm

Require cargo

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# AC-006: N=10 concurrent sends with invalid_grant -> exactly 1 refresh hit.
# RefreshState caches the failure; all 10 callers surface NotAuthenticated.
# No thundering-herd retry storm. Verified via Mock::expect(1).

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'cargo test --test oauth_refresh_integration test_concurrent_invalid_grant_no_thundering_herd -- --nocapture 2>&1 | tail -10'
Enter
Sleep 12s

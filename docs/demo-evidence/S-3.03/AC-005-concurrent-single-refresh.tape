Output docs/demo-evidence/S-3.03/AC-005-concurrent-single-refresh.gif
Output docs/demo-evidence/S-3.03/AC-005-concurrent-single-refresh.webm

Require cargo

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# AC-005: N=10 concurrent sends -> exactly 1 refresh HTTP request.
# Mock::expect(1) on refresh endpoint; verified on MockServer drop.
# N-1 waiters use in-memory access token from RefreshState.

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'cargo test --test oauth_refresh_integration test_concurrent_sends_single_refresh_via_coordinator -- --nocapture 2>&1 | tail -10'
Enter
Sleep 12s

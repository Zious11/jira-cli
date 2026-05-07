# Combined: all three S-0.01 ACs pass in one cargo test run
# Runs the full issue_open integration test suite showing 3 passed, 0 failed.
# Covers AC-001 (OAuth URL), AC-002 (api-token regression), AC-003 (no double-slash).

Output docs/demo-evidence/S-0.01/AC-combined-all-three-pass.gif
Output docs/demo-evidence/S-0.01/AC-combined-all-three-pass.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1000
Set Height 500
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

Type "# S-0.01 combined demo: 3/3 ACs pass"
Enter
Sleep 500ms

Type "cargo test --test issue_open -- --test-threads=1"
Enter
Sleep 30s

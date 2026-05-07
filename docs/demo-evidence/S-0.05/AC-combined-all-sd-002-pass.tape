# Combined: all 5 SD-002 tests pass (5/5 green)
# Shows the full auth_header_release_gate test suite running to completion.
#
# Expected: 5 passed; 0 failed

Output docs/demo-evidence/S-0.05/AC-combined-all-sd-002-pass.gif
Output docs/demo-evidence/S-0.05/AC-combined-all-sd-002-pass.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.05-jr-auth-header-cfg-test-gate"
Enter
Sleep 500ms
Show

Type "# S-0.05 SD-002 gate: all 5 tests green"
Enter
Sleep 300ms

Type "cargo test --test auth_header_release_gate -- --nocapture --test-threads=1"
Enter
Sleep 60s

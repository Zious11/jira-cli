# AC-003 (BC-1.4.028): partial credential state (access token present, refresh absent)
# returns an error — never silently used.
#   test_s_1_08_ac003_partial_state_error_message_contains_partial
#
# Output: AC-003-partial-state-error.gif + .webm

Output docs/demo-evidence/S-1.08/AC-003-partial-state-error.gif
Output docs/demo-evidence/S-1.08/AC-003-partial-state-error.webm

Require cargo

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 30
Set TypingSpeed 40ms

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.08-keychain-roundtrip-holdout"
Enter
Sleep 500ms
Show

Type "# AC-003 (BC-1.4.028): partial credential state (one token missing) returns error"
Enter
Sleep 300ms

Type "cargo test --lib test_s_1_08_ac003 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 30s

Type "# Result: 1 passed — partial token state never silently accepted"
Enter
Sleep 2s

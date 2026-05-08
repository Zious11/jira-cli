# AC-004: Retry-After 0 boundary value preserved as Some(0)
# Zero is a valid value (no delay before retry), not coerced to None
# or DEFAULT_RETRY_SECS
#
# Output: AC-004-retry-after-0-boundary.gif + .webm

Output docs/demo-evidence/S-1.07/AC-004-retry-after-0-boundary.gif
Output docs/demo-evidence/S-1.07/AC-004-retry-after-0-boundary.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.07-rate-limit-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-004: Retry-After: 0 boundary — preserved as Some(0), not coerced to None"
Enter
Sleep 300ms

Type "cargo test --test rate_limit_holdouts test_s_1_07_retry_after_0 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — boundary value Some(0) confirmed"
Enter
Sleep 2s

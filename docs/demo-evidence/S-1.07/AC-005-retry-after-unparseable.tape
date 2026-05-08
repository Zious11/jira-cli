# AC-005: Retry-After "abc" unparseable returns None
# DEFAULT_RETRY_SECS applied at call site, not inside from_headers.
# HTTP-date format also falls through to None per NFR-SCA-1.
#
# Output: AC-005-retry-after-unparseable.gif + .webm

Output docs/demo-evidence/S-1.07/AC-005-retry-after-unparseable.gif
Output docs/demo-evidence/S-1.07/AC-005-retry-after-unparseable.webm

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

Type "# AC-005: Retry-After: abc (non-integer) → None; DEFAULT_RETRY_SECS applied at call site"
Enter
Sleep 300ms

Type "cargo test --test rate_limit_holdouts test_s_1_07_retry_after_unparseable -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — parse failure returns None, not DEFAULT_RETRY_SECS"
Enter
Sleep 2s

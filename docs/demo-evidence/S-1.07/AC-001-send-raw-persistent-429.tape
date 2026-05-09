# AC-001 / H-013: send_raw persistent 429 retry exhaustion
# Covers: library assertion (call count + status) + process stderr "gave up" warning
# Both test_s_1_07_h_013_send_raw_persistent_429_returns_429_after_max_retries
# and test_s_1_07_h_013_send_raw_gave_up_warning_in_stderr pass.
#
# Output: AC-001-send-raw-persistent-429.gif + .webm

Output docs/demo-evidence/S-1.07/AC-001-send-raw-persistent-429.gif
Output docs/demo-evidence/S-1.07/AC-001-send-raw-persistent-429.webm

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

Type "# AC-001 / H-013: send_raw persistent 429 — retry exhaustion (MAX_RETRIES=3)"
Enter
Sleep 300ms

Type "cargo test --test rate_limit_holdouts test_s_1_07_h_013_send_raw -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 2 passed — library assertion + process stderr 'gave up' warning verified"
Enter
Sleep 2s

# AC-003: send recovers after 2 retries (429, 429, 200) — regression guard
# Standard send path retries on 429 and succeeds on eventual 200.
# Sequence: 429 → 429 → 200 (exactly 3 HTTP calls via wiremock .expect())
#
# Output: AC-003-send-recovers-after-2-retries.gif + .webm

Output docs/demo-evidence/S-1.07/AC-003-send-recovers-after-2-retries.gif
Output docs/demo-evidence/S-1.07/AC-003-send-recovers-after-2-retries.webm

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

Type "# AC-003: send recovers after 2 retries — sequence 429 → 429 → 200 (3 HTTP calls)"
Enter
Sleep 300ms

Type "cargo test --test rate_limit_holdouts test_s_1_07_h_013_send_recovers -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — recovery regression guard verified (wiremock call-count enforced)"
Enter
Sleep 2s

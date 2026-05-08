# AC-002 / H-027: Retry-After 86400 — no cap (KNOWN-GAP pin for S-3.07)
# RateLimitInfo::from_headers returns Some(86400) without upper-bound cap
# KNOWN-GAP: when NFR-R-NEW-1 lands in S-3.07, assertion changes to Some(60)
#
# Output: AC-002-retry-after-86400-no-cap.gif + .webm

Output docs/demo-evidence/S-1.07/AC-002-retry-after-86400-no-cap.gif
Output docs/demo-evidence/S-1.07/AC-002-retry-after-86400-no-cap.webm

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

Type "# AC-002 / H-027: Retry-After: 86400 — preserved as Some(86400), no upper-bound cap"
Enter
Sleep 300ms

Type "cargo test --test rate_limit_holdouts test_s_1_07_h_027 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — KNOWN-GAP pin; S-3.07 changes assertion to Some(60)"
Enter
Sleep 2s

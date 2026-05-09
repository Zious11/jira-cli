# COMBINED: Full rate-limit holdout suite — 6/6 green
#
# AC-001 / H-013: send_raw persistent 429 (library + process) — 2 tests
# AC-002 / H-027: Retry-After 86400 no cap (KNOWN-GAP pin) — 1 test
# AC-003: send recovers after 2 retries — 1 test
# AC-004: Retry-After 0 boundary — 1 test
# AC-005: Retry-After unparseable returns None — 1 test
#
# Expected: 6 passed; 0 failed

Output docs/demo-evidence/S-1.07/COMBINED-all-6-tests-green.gif
Output docs/demo-evidence/S-1.07/COMBINED-all-6-tests-green.webm

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

Type "# S-1.07: rate-limit regression holdout suite — 6/6 tests"
Enter
Sleep 300ms

Type "cargo test --test rate_limit_holdouts -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 90s

Type "# Result: 6 passed; 0 failed — full S-1.07 holdout suite green"
Enter
Sleep 2s

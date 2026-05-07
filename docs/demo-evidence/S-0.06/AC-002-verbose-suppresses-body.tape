# AC-002 / H-NEW-VERBOSE-002: --verbose alone suppresses body bytes
# test_sd_003_verbose_alone_suppresses_body_bytes
#
# Verifies that --verbose WITHOUT --verbose-bodies does NOT print body bytes
# (sentinel PII strings must NOT appear in stderr), and the suppression hint
# IS present so users know how to migrate to --verbose-bodies.
#
# Pre-fix FAILS: current --verbose prints the full response body, so
# sentinel@example.test and acc_sentinel_42 appear in stderr.
# Post-fix PASSES: bodies are suppressed; suppression hint is emitted.
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.06/AC-002-verbose-suppresses-body.gif
Output docs/demo-evidence/S-0.06/AC-002-verbose-suppresses-body.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.06-verbose-bodies-flag-and-pii-warning"
Enter
Sleep 500ms
Show

Type "# AC-002 / H-NEW-VERBOSE-002: --verbose alone suppresses body bytes"
Enter
Sleep 300ms

Type "cargo test --test verbose_bodies test_sd_003_verbose_alone_suppresses_body_bytes -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 1 passed; 0 failed — body sentinel strings absent from stderr"
Enter
Sleep 2s

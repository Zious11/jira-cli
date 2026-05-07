# AC-003: --verbose + --verbose-bodies combined prints method+URL, PII warning, AND body
# test_sd_003_verbose_plus_verbose_bodies_prints_body
#
# Verifies that combining --verbose and --verbose-bodies stacks correctly:
# - [verbose] method+URL line appears (from --verbose)
# - 3-line PII warning appears (from --verbose-bodies)
# - Body sentinel strings appear in stderr (from --verbose-bodies)
#
# Pre-fix FAILS: clap parse error "unexpected argument '--verbose-bodies'"
# Post-fix PASSES: both flags compose orthogonally
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.06/AC-003-verbose-stacks-with-bodies.gif
Output docs/demo-evidence/S-0.06/AC-003-verbose-stacks-with-bodies.webm

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

Type "# AC-003: --verbose + --verbose-bodies stacks: method+URL, PII warning, body bytes"
Enter
Sleep 300ms

Type "cargo test --test verbose_bodies test_sd_003_verbose_plus_verbose_bodies_prints_body -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 1 passed; 0 failed — all three outputs confirmed"
Enter
Sleep 2s

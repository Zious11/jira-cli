# AC-004: --verbose-bodies alone prints body + warning, NOT method+URL line
# test_sd_003_verbose_bodies_alone_prints_body_without_url_line
#
# Verifies that --verbose-bodies without --verbose:
# - DOES print body bytes (sentinel strings present in stderr)
# - DOES print PII warning
# - Does NOT print the [verbose] GET/POST/... method+URL line
#   (that requires --verbose; the two flags are orthogonal)
#
# Pre-fix FAILS: clap parse error "unexpected argument '--verbose-bodies'"
# Post-fix PASSES: bodies printed with warning, no method+URL line
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.06/AC-004-verbose-bodies-alone.gif
Output docs/demo-evidence/S-0.06/AC-004-verbose-bodies-alone.webm

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

Type "# AC-004: --verbose-bodies alone: body + warning, no method+URL line"
Enter
Sleep 300ms

Type "cargo test --test verbose_bodies test_sd_003_verbose_bodies_alone_prints_body_without_url_line -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 1 passed; 0 failed — orthogonality of --verbose and --verbose-bodies confirmed"
Enter
Sleep 2s

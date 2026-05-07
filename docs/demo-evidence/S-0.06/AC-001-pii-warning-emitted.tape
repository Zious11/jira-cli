# AC-001 / H-NEW-VERBOSE-001: --verbose-bodies emits PII warning
# test_sd_003_verbose_bodies_emits_pii_warning
#
# Verifies that when --verbose-bodies is passed, stderr contains all three
# PII warning lines before any body content is printed.
#
# Pre-fix FAILS: clap parse error "unexpected argument '--verbose-bodies'"
# Post-fix PASSES: 3-line PII warning present in stderr
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.06/AC-001-pii-warning-emitted.gif
Output docs/demo-evidence/S-0.06/AC-001-pii-warning-emitted.webm

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

Type "# AC-001 / H-NEW-VERBOSE-001: --verbose-bodies emits 3-line PII warning"
Enter
Sleep 300ms

Type "cargo test --test verbose_bodies test_sd_003_verbose_bodies_emits_pii_warning -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 1 passed; 0 failed — PII warning confirmed in stderr"
Enter
Sleep 2s

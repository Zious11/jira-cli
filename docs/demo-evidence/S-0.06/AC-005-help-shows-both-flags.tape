# AC-005: jr --help lists both --verbose and --verbose-bodies
# test_sd_003_help_mentions_verbose_bodies_flag
#
# Verifies that jr --help stdout contains both --verbose and --verbose-bodies
# as distinct flags, and that the --verbose description references --verbose-bodies
# so users know how to migrate.
#
# Pre-fix FAILS: --verbose-bodies absent from help text
# Post-fix PASSES: both flags listed with correct descriptions
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.06/AC-005-help-shows-both-flags.gif
Output docs/demo-evidence/S-0.06/AC-005-help-shows-both-flags.webm

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

Type "# AC-005: jr --help lists --verbose and --verbose-bodies"
Enter
Sleep 300ms

Type "cargo test --test verbose_bodies test_sd_003_help_mentions_verbose_bodies_flag -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 1 passed; 0 failed — both flags present in --help output"
Enter
Sleep 2s

# AC-006: CHANGELOG.md contains BREAKING CHANGE entry for --verbose body suppression
# test_sd_003_changelog_has_breaking_change_entry
#
# Verifies that CHANGELOG.md contains:
# - "BREAKING CHANGE" or "BREAKING:" string
# - "--verbose-bodies" (migration target)
# - "--verbose" (the changed flag)
#
# Pre-fix FAILS: CHANGELOG.md absent or missing BREAKING CHANGE entry
# Post-fix PASSES: CHANGELOG.md has the required entry
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.06/AC-006-changelog-breaking-change.gif
Output docs/demo-evidence/S-0.06/AC-006-changelog-breaking-change.webm

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

Type "# AC-006: CHANGELOG.md has BREAKING CHANGE entry for --verbose body suppression"
Enter
Sleep 300ms

Type "cargo test --test verbose_bodies test_sd_003_changelog_has_breaking_change_entry -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 1 passed; 0 failed — BREAKING CHANGE entry confirmed in CHANGELOG.md"
Enter
Sleep 2s

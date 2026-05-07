# Combined: All 8 BC-6.3.001 tests pass
# Runs all 8 multi_profile_fields integration + unit tests in one harness pass.
# Demonstrates complete coverage of the multi-profile field routing fix (S-0.04).
# Holdout H-NEW-MP-001: MUST-FAIL at pre-fix HEAD, MUST-PASS at b304442.

Output docs/demo-evidence/S-0.04/AC-combined-all-bc-6-3-001-pass.gif
Output docs/demo-evidence/S-0.04/AC-combined-all-bc-6-3-001-pass.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1200
Set Height 500
Set FontFamily "Menlo"
Set Theme "Dracula"
Set Padding 24
Set TypingSpeed 40ms
Set PlaybackSpeed 1

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.04-multi-profile-fields-active"
Enter
Sleep 500ms
Show

Type "# Combined: all 8 BC-6.3.001 tests — expect 8/8 PASS"
Enter
Sleep 500ms

Type "cargo test --test multi_profile_fields -- --nocapture --test-threads=1 2>&1 | grep -E 'running|test .* \.\.\.|test result'"
Enter
Sleep 60s

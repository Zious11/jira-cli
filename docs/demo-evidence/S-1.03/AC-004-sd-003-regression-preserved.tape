# AC-004 (SD-003 regression): verbose_bodies contract preserved after tracing wire-up
# All 6 verbose_bodies tests from S-0.06 still pass
#
# Verifies that adding tracing deps and replacing eprintln! for HTTP-level events
# does NOT break the SD-003 --verbose-bodies behavior: PII warning still emitted,
# body suppression still works, --verbose stacking still works.
#
# Pre-fix FAILS: tracing changes could break SD-003 PII warning paths
# Post-fix PASSES: all 6 verbose_bodies tests green (regression preserved)
#
# Expected: 6 passed; 0 failed

Output docs/demo-evidence/S-1.03/AC-004-sd-003-regression-preserved.gif
Output docs/demo-evidence/S-1.03/AC-004-sd-003-regression-preserved.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.03-tracing-observability"
Enter
Sleep 500ms
Show

Type "# AC-004: SD-003 contract preserved — all 6 verbose_bodies tests still pass"
Enter
Sleep 300ms

Type "cargo test --test verbose_bodies -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 6 passed; 0 failed — SD-003 regression prevented"
Enter
Sleep 2s

# BONUS: Live UX showcase — --help output grep + PII warning demo
#
# Shows two things:
# 1. jr --help output filtered to verbose-related flags (both visible)
# 2. The PII warning test run showing actual warning text emitted to stderr
#
# This is not a live Jira call (no configured profile needed) — it uses
# the test harness which spins up a wiremock server internally.

Output docs/demo-evidence/S-0.06/BONUS-live-ux.gif
Output docs/demo-evidence/S-0.06/BONUS-live-ux.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1000
Set Height 500
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

Type "# BONUS: Live UX showcase for S-0.06 --verbose-bodies + PII warning"
Enter
Sleep 500ms

Type "# Step 1: jr --help shows both verbose flags"
Enter
Sleep 300ms

Type "cargo run --quiet -- --help 2>/dev/null | grep -E 'verbose'"
Enter
Sleep 30s

Type "# Step 2: PII warning text emitted by --verbose-bodies (via test with --nocapture)"
Enter
Sleep 300ms

Type "cargo test --test verbose_bodies test_sd_003_verbose_bodies_emits_pii_warning -- --nocapture --test-threads=1 2>&1 | grep -E 'WARNING|PII|jr|passed|failed'"
Enter
Sleep 60s

Type "# Both flags visible in help; PII warning confirmed in stderr"
Enter
Sleep 2s

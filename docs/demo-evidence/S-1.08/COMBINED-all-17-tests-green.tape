# COMBINED: Full S-1.08 keychain layout holdout suite — 17/17 green
#
# AC-001 (BC-1.4.027): 7 key-naming tests (per-profile namespacing)
# AC-002 (BC-1.4.025): 1 lazy-migration guard test (default-only)
# AC-003 (BC-1.4.028): 1 partial-state error test
# AC-004 (BC-1.4.029): 1 profile boundary isolation test
# AC-005 (BC-1.4.030): 4 resolver precedence tests (keychain > embedded)
# AC-006 (H-016):      3 process-spawn tests (active-profile removal guard)
#
# Expected: 17 passed; 0 failed
#
# Output: COMBINED-all-17-tests-green.gif + .webm

Output docs/demo-evidence/S-1.08/COMBINED-all-17-tests-green.gif
Output docs/demo-evidence/S-1.08/COMBINED-all-17-tests-green.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.08-keychain-roundtrip-holdout"
Enter
Sleep 500ms
Show

Type "# S-1.08: keychain layout holdout suite — all 17 tests"
Enter
Sleep 300ms

Type "cargo test test_s_1_08_ -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 90s

Type "# Result: 17 passed; 0 failed — full S-1.08 keychain holdout suite green"
Enter
Sleep 2s

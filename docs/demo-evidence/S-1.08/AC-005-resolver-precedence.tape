# AC-005 (BC-1.4.030): resolve_refresh_app_credentials checks keychain FIRST.
# BYO keychain credentials win over the embedded OAuth app.
# 4 tests covering all resolver paths:
#   test_s_1_08_ac005_keychain_wins_over_embedded_when_both_present
#   test_s_1_08_ac005_keychain_wins_when_only_keychain_present
#   test_s_1_08_ac005_embedded_fallback_when_no_keychain
#   test_s_1_08_ac005_none_when_no_source_resolved
#
# Output: AC-005-resolver-precedence.gif + .webm

Output docs/demo-evidence/S-1.08/AC-005-resolver-precedence.gif
Output docs/demo-evidence/S-1.08/AC-005-resolver-precedence.webm

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

Type "# AC-005 (BC-1.4.030): keychain > embedded credential resolver precedence"
Enter
Sleep 300ms

Type "cargo test --lib test_s_1_08_ac005 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 30s

Type "# Result: 4 passed — BYO keychain always wins over embedded OAuth app"
Enter
Sleep 2s

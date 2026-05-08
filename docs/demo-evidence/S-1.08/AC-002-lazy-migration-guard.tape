# AC-002 (BC-1.4.025): lazy migration guard is default-only; non-default profiles
# never inherit legacy flat OAuth keys.
#   test_s_1_08_ac002_lazy_migration_guard_sentinel_is_default
#
# Output: AC-002-lazy-migration-guard.gif + .webm

Output docs/demo-evidence/S-1.08/AC-002-lazy-migration-guard.gif
Output docs/demo-evidence/S-1.08/AC-002-lazy-migration-guard.webm

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

Type "# AC-002 (BC-1.4.025): lazy migration is 'default' profile only — guard verified"
Enter
Sleep 300ms

Type "cargo test --lib test_s_1_08_ac002 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 30s

Type "# Result: 1 passed — 'if profile == default' guard prevents non-default migration"
Enter
Sleep 2s

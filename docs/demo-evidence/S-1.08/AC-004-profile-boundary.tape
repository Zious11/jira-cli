# AC-004 (BC-1.4.029): namespaced keys for 'sandbox' never alias legacy flat keys.
# sandbox:oauth-access-token is distinct from legacy oauth-access-token.
#   test_s_1_08_ac004_namespaced_key_never_aliases_legacy_key
#
# Output: AC-004-profile-boundary.gif + .webm

Output docs/demo-evidence/S-1.08/AC-004-profile-boundary.gif
Output docs/demo-evidence/S-1.08/AC-004-profile-boundary.webm

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

Type "# AC-004 (BC-1.4.029): per-profile namespaced keys never alias legacy flat keys"
Enter
Sleep 300ms

Type "cargo test --lib test_s_1_08_ac004 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 30s

Type "# Result: 1 passed — sandbox namespace isolated from default profile legacy keys"
Enter
Sleep 2s

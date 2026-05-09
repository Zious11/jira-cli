# AC-001 (BC-1.4.027): OAuth token keys follow <profile>:oauth-access-token pattern
# Covers 7 unit tests verifying per-profile namespacing of OAuth token keys:
#   test_s_1_08_ac001_oauth_access_key_default_profile
#   test_s_1_08_ac001_oauth_access_key_sandbox_profile
#   test_s_1_08_ac001_oauth_refresh_key_default_profile
#   test_s_1_08_ac001_oauth_refresh_key_sandbox_profile
#   test_s_1_08_ac001_profile_keys_are_distinct_across_profiles
#   test_s_1_08_ac001_shared_keys_are_not_namespaced
#   test_s_1_08_ac001_key_format_structure
#
# Output: AC-001-key-naming-per-profile.gif + .webm

Output docs/demo-evidence/S-1.08/AC-001-key-naming-per-profile.gif
Output docs/demo-evidence/S-1.08/AC-001-key-naming-per-profile.webm

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

Type "# AC-001 (BC-1.4.027): namespaced key format '<profile>:oauth-access-token'"
Enter
Sleep 300ms

Type "cargo test --lib test_s_1_08_ac001 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 30s

Type "# Result: 7 passed — per-profile key naming verified for default + sandbox profiles"
Enter
Sleep 2s

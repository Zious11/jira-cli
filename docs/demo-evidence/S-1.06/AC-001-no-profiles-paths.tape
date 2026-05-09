# AC-001 (H-001 + H-002): no-profiles paths
# test_s_1_06_h_001_auth_status_no_profiles
# test_s_1_06_h_002_auth_list_json_no_profiles
#
# Pins: auth status on fresh install exits 0 + "No profiles configured";
#       auth list --output json on fresh install exits 0 + `[]`.
#
# Expected: 2 passed; 0 failed

Output docs/demo-evidence/S-1.06/AC-001-no-profiles-paths.gif
Output docs/demo-evidence/S-1.06/AC-001-no-profiles-paths.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.06-oauth-flow-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-001: auth status + auth list --output json on fresh install"
Enter
Sleep 300ms

Type "cargo test --test oauth_flow_holdouts test_s_1_06_h_00 -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 2 passed; 0 failed — no-profiles paths green"
Enter
Sleep 2s

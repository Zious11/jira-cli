# AC-001: --profile sandbox uses sandbox field IDs (BC-6.3.001)
# Given two profiles where prod has story_points_field_id = "customfield_10005"
# and sandbox has story_points_field_id = "customfield_10099",
# running jr --profile sandbox issue create --points 5 MUST POST customfield_10099.
# Pre-fix: code reads config.global.fields (empty/None) — sandbox field not sent.
# Post-fix: code reads config.active_profile() — sandbox field used correctly.

Output docs/demo-evidence/S-0.04/AC-001-sandbox-profile-fields.gif
Output docs/demo-evidence/S-0.04/AC-001-sandbox-profile-fields.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.04-multi-profile-fields-active"
Enter
Sleep 500ms
Show

Type "# AC-001: --profile sandbox uses sandbox story-points field ID"
Enter
Sleep 500ms

Type "cargo test --test multi_profile_fields test_bc_6_3_001_sandbox_profile_uses_sandbox_story_points_field_id -- --nocapture --test-threads=1"
Enter
Sleep 60s

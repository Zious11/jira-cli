# AC-002: --points column populated after save round-trip (BC-6.3.001)
# After a save round-trip (legacy [fields] block absent, field IDs only in
# [profiles.default]), jr issue list --points must show the Points column.
# Pre-fix: config.global.fields.story_points_field_id is None — column blank/absent.
# Post-fix: config.active_profile().story_points_field_id returns correct value.

Output docs/demo-evidence/S-0.04/AC-002-points-column-after-save.gif
Output docs/demo-evidence/S-0.04/AC-002-points-column-after-save.webm

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

Type "# AC-002: Points column present after save round-trip (no legacy [fields] block)"
Enter
Sleep 500ms

Type "cargo test --test multi_profile_fields test_bc_6_3_001_points_column_present_after_save_round_trip -- --nocapture --test-threads=1"
Enter
Sleep 60s

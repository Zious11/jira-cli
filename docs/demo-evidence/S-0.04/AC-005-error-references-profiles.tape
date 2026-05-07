# AC-005: error message references [profiles.<name>] not [fields] (BC-6.3.001)
# When story_points_field_id is missing for the active profile,
# the error must say "set story_points_field_id under [profiles.<name>]"
# not the deprecated "set story_points_field_id under [fields]".
# Pre-fix: error says "[fields]"; Post-fix: error says "[profiles.<name>]".

Output docs/demo-evidence/S-0.04/AC-005-error-references-profiles.gif
Output docs/demo-evidence/S-0.04/AC-005-error-references-profiles.webm

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

Type "# AC-005: error message references [profiles.<name>] not deprecated [fields]"
Enter
Sleep 500ms

Type "cargo test --test multi_profile_fields test_bc_6_3_001_error_message_references_profiles_section_not_fields -- --nocapture --test-threads=1"
Enter
Sleep 60s

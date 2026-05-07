# AC-006: list.rs resolve_show_points warning updated (BC-6.3.001)
# When --points is passed but story_points_field_id is missing from the active
# profile, the warning on stderr must say
# "set story_points_field_id under [profiles.<name>]"
# not the old "[fields].story_points_field_id in ..." wording.
# Pre-fix: warning says "[fields]"; Post-fix: warning says "[profiles.<name>]".

Output docs/demo-evidence/S-0.04/AC-006-list-warning-references-profiles.gif
Output docs/demo-evidence/S-0.04/AC-006-list-warning-references-profiles.webm

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

Type "# AC-006: list warning references [profiles.<name>] not deprecated [fields]"
Enter
Sleep 500ms

Type "cargo test --test multi_profile_fields test_bc_6_3_001_list_points_warning_references_profiles_section -- --nocapture --test-threads=1"
Enter
Sleep 60s

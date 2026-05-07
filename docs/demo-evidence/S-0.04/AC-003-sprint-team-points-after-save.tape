# AC-003: sprint current shows team/points after save round-trip (BC-6.3.001)
# jr sprint current must include Team and Points columns when field IDs live only
# in [profiles.default] (no [fields] block present).
# Pre-fix: both fields are None; sprint issues API call does not request them.
# Post-fix: code reads config.active_profile() — both columns populated.

Output docs/demo-evidence/S-0.04/AC-003-sprint-team-points-after-save.gif
Output docs/demo-evidence/S-0.04/AC-003-sprint-team-points-after-save.webm

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

Type "# AC-003: sprint current shows Team and Points after save round-trip"
Enter
Sleep 500ms

Type "cargo test --test multi_profile_fields test_bc_6_3_001_sprint_current_shows_team_and_points_after_save_round_trip -- --nocapture --test-threads=1"
Enter
Sleep 60s

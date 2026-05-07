# AC-004: board view shows team after save round-trip (BC-6.3.001)
# jr board view must include a Team column when team_field_id lives only in
# [profiles.default] (no [fields] block present).
# Pre-fix: config.global.fields.team_field_id is None — Team column absent.
# Post-fix: code reads config.active_profile() — Team column and resolved name shown.

Output docs/demo-evidence/S-0.04/AC-004-board-team-after-save.gif
Output docs/demo-evidence/S-0.04/AC-004-board-team-after-save.webm

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

Type "# AC-004: board view shows Team column after save round-trip"
Enter
Sleep 500ms

Type "cargo test --test multi_profile_fields test_bc_6_3_001_board_view_shows_team_after_save_round_trip -- --nocapture --test-threads=1"
Enter
Sleep 60s

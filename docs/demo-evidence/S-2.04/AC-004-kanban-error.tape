# AC-004 / H-042 / BC-5.2.001: jr sprint list --board 1 on a kanban board → non-zero exit + exact error message
# Covers: test_s_2_04_h_042_bc_5_2_001_kanban_board_errors_on_sprint_list
# Pins: kanban board type check in src/cli/sprint.rs — exact literal "Sprint commands are only available for scrum boards"
#
# The test spawns `jr sprint list --board 1` against a wiremock board config endpoint
# that returns type="kanban". Sprint commands check board type and reject kanban boards
# with the exact error string. A future wording change would fail this holdout.
#
# Output: AC-004-kanban-error.gif + AC-004-kanban-error.webm

Output docs/demo-evidence/S-2.04/AC-004-kanban-error.gif
Output docs/demo-evidence/S-2.04/AC-004-kanban-error.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-2.04-bc-5-boards-sprints-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-004 / H-042: sprint list on kanban board → exact error + non-zero exit"
Enter
Sleep 300ms

Type "cargo test --test boards_sprints_holdouts test_s_2_04_h_042_bc_5_2_001_kanban_board_errors_on_sprint_list -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 30s

Type "# Result: 1 passed — stderr contains 'Sprint commands are only available for scrum boards', exit non-zero"
Enter
Sleep 2s

# AC-003 / BC-X.5.009: invalid duration string "1z" rejected before network call
# Covers: test_s_2_06_ac_003_bc_x_5_009_invalid_duration_rejected_before_network
# Pins: parse_duration_validate error message format (contains Nw/Nd/Nh/Nm syntax hint)
#       early-exit in src/cli/worklog.rs::handle_add before any POST is made
#       exit code 64 (JrError::UserError) for bad duration input
#
# The test spawns `jr worklog add PROJ-1 "1z"` against a wiremock that expects zero
# POST calls. The validator catches the unknown unit 'z' before any network call.
# Exit code 64, stderr contains "Nw Nd Nh Nm" syntax hint, zero POSTs made.
#
# Output: AC-003-invalid-duration-rejected.gif + AC-003-invalid-duration-rejected.webm

Output docs/demo-evidence/S-2.06/AC-003-invalid-duration-rejected.gif
Output docs/demo-evidence/S-2.06/AC-003-invalid-duration-rejected.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-2.06-worklog-time-spent-passthrough"
Enter
Sleep 500ms
Show

Type "# AC-003 / BC-X.5.009: invalid duration '1z' → exit 64 + syntax hint, zero POSTs"
Enter
Sleep 300ms

Type "cargo test --test worklog_duration_holdouts test_s_2_06_ac_003_bc_x_5_009_invalid_duration_rejected_before_network -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 30s

Type "# Result: 1 passed — exit 64, stderr contains 'Nw Nd Nh Nm', zero POST requests made"
Enter
Sleep 2s

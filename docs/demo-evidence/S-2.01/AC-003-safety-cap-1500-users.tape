# AC-003 / H-032 / BC-X.2.006: stateless infinite 100-user mock → safety cap fires at 1500 users
# Covers: test_s_2_01_h_032_bc_x_2_006_safety_cap_fires_at_1500_with_warning
# Pins: USER_PAGINATION_SAFETY_CAP=15 in src/api/jira/users.rs
#
# Output: AC-003-safety-cap-1500-users.gif + .webm

Output docs/demo-evidence/S-2.01/AC-003-safety-cap-1500-users.gif
Output docs/demo-evidence/S-2.01/AC-003-safety-cap-1500-users.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-2.01-bc-2-issue-read-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-003 / H-032: safety cap fires at 1500 users (15 pages × 100) with stderr warning"
Enter
Sleep 300ms

Type "cargo test --test issue_read_holdouts test_s_2_01_h_032 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — 1500 users collected, safety cap warning emitted"
Enter
Sleep 2s

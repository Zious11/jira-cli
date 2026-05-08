# AC-002 / H-031 / BC-X.2.005: short page (35) on page 2 → startAt advances by PAGE_SIZE (100)
# Covers: test_s_2_01_h_031_bc_x_2_005_short_page_advances_by_page_size_not_returned_count
# Pins: JRACLOUD-71293 fixed-window pagination in src/api/jira/users.rs::search_users_all
#
# Output: AC-002-short-page-advances-by-page-size.gif + .webm

Output docs/demo-evidence/S-2.01/AC-002-short-page-advances-by-page-size.gif
Output docs/demo-evidence/S-2.01/AC-002-short-page-advances-by-page-size.webm

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

Type "# AC-002 / H-031: short page (35) on page 2 → fixed-window advance by PAGE_SIZE (100)"
Enter
Sleep 300ms

Type "cargo test --test issue_read_holdouts test_s_2_01_h_031 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — 235 users (100+35+100), startAt advanced correctly"
Enter
Sleep 2s

# AC-006 / H-035 / BC-2.1.001: combined filters compose JQL without panic → 5-element JSON array
# Covers: test_s_2_01_h_035_bc_2_1_001_all_filters_combined_compose_jql_without_panic
# Pins: filter composition path in src/cli/issue/list.rs::handle_list
#
# Output: AC-006-all-filters-combined-no-panic.gif + .webm

Output docs/demo-evidence/S-2.01/AC-006-all-filters-combined-no-panic.gif
Output docs/demo-evidence/S-2.01/AC-006-all-filters-combined-no-panic.webm

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

Type "# AC-006 / H-035: --assignee + --created-after + --status + --team compose JQL, exit 0, 5 issues"
Enter
Sleep 300ms

Type "cargo test --test issue_read_holdouts test_s_2_01_h_035 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — 5-element JSON array returned, no panic"
Enter
Sleep 2s

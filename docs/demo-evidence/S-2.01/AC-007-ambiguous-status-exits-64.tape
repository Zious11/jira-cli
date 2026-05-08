# AC-007 / H-021 / BC-2.1.007: --status prog ambiguous → exit 64, JQL not invoked
# Covers: test_s_2_01_h_021_bc_2_1_007_ambiguous_status_exits_64_no_jql_call
# Pins: partial_match disambiguation in src/cli/issue/list.rs + src/partial_match.rs
#
# Output: AC-007-ambiguous-status-exits-64.gif + .webm

Output docs/demo-evidence/S-2.01/AC-007-ambiguous-status-exits-64.gif
Output docs/demo-evidence/S-2.01/AC-007-ambiguous-status-exits-64.webm

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

Type "# AC-007 / H-021: --status prog ambiguous → exit 64, 'Ambiguous status' in stderr, JQL not called"
Enter
Sleep 300ms

Type "cargo test --test issue_read_holdouts test_s_2_01_h_021 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — exit 64 (UserError), ambiguous resolution short-circuits before JQL"
Enter
Sleep 2s

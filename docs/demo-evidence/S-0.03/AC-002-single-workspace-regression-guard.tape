# AC-002: Single-workspace tenants unchanged (regression guard — BC-4.3.001)
# When all assets are from one workspace, output is identical to pre-fix behavior.
# Verifies the composite key change does not break single-workspace tenants.

Output docs/demo-evidence/S-0.03/AC-002-single-workspace-regression-guard.gif
Output docs/demo-evidence/S-0.03/AC-002-single-workspace-regression-guard.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.03-multi-workspace-asset-hashmap-key"
Enter
Sleep 500ms
Show

Type "# AC-002: Single-workspace regression guard — output unchanged by composite key fix"
Enter
Sleep 500ms

Type "cargo test --test issue_list_assets test_bc_4_3_001_single_workspace_regression_guard -- --nocapture --test-threads=1"
Enter
Sleep 30s

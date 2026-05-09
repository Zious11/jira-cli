Output docs/demo-evidence/S-3.04/AC-003-single-resource-no-regression.gif
Output docs/demo-evidence/S-3.04/AC-003-single-resource-no-regression.webm

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 600
Set FontFamily "Menlo"
Set Theme "TokyoNight"
Set Padding 16

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.04"
Enter
Sleep 500ms
Show

Type 'cargo test --test multi_cloudid_disambiguation test_single_resource_no_regression_single_org_path 2>&1 | tail -15'
Enter
Sleep 60s

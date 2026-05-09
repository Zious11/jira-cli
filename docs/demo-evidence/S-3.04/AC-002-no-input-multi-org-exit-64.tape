Output docs/demo-evidence/S-3.04/AC-002-no-input-multi-org-exit-64.gif
Output docs/demo-evidence/S-3.04/AC-002-no-input-multi-org-exit-64.webm

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

Type 'cargo test --test multi_cloudid_disambiguation test_no_input_multi_org_exits_64_with_actionable_error -- --nocapture 2>&1 | tail -25'
Enter
Sleep 60s

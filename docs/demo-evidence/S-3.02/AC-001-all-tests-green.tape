Output docs/demo-evidence/S-3.02/AC-001-all-tests-green.gif
Output docs/demo-evidence/S-3.02/AC-001-all-tests-green.webm

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 600
Set FontFamily "Menlo"
Set Theme "TokyoNight"
Set Padding 16

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.02"
Enter
Sleep 500ms
Show

Type 'cargo test --test asset_holdouts 2>&1 | tail -5'
Enter
Sleep 60s

Type 'cargo test --lib 2>&1 | tail -5'
Enter
Sleep 120s

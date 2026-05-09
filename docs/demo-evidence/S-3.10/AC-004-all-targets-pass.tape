Output AC004_all_targets_pass.gif
Output AC004_all_targets_pass.webm

Require cargo

Set FontFamily "Menlo"
Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Dracula"
Set Padding 20

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.10"
Enter
Sleep 1s
Show

Type "cargo test --all-targets 2>&1 | tail -20"
Enter
Sleep 60s

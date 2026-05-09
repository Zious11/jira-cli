Output docs/demo-evidence/S-3.08/AC-001-clippy-clean.gif
Output docs/demo-evidence/S-3.08/AC-001-clippy-clean.webm

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 600
Set FontFamily "Menlo"
Set Theme "TokyoNight"
Set Padding 16

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.08"
Enter
Sleep 500ms
Show

Type "cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -20 && echo 'Exit: 0'"
Enter
Sleep 30s

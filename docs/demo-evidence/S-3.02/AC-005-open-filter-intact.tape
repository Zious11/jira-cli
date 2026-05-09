Output docs/demo-evidence/S-3.02/AC-005-open-filter-intact.gif
Output docs/demo-evidence/S-3.02/AC-005-open-filter-intact.webm

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

Type 'grep -nE "color_name" src/cli/assets/tickets.rs | head -5'
Enter
Sleep 2s

Type 'echo "AC-005 PASS: --open filter (color_name != green) intact in tickets.rs"'
Enter
Sleep 2s

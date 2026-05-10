Output docs/demo-evidence/issue-284/D-001-no-parent-in-help.gif
Output docs/demo-evidence/issue-284/D-001-no-parent-in-help.webm

Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

Hide
Type "export PATH=/Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-284/target/release:$PATH"
Enter
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-284"
Enter
Type "clear"
Enter
Show

Type 'jr issue edit --help 2>&1 | grep -A 2 "no-parent"'
Enter
Sleep 3s

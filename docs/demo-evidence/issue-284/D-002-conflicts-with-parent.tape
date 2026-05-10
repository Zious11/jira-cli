Output docs/demo-evidence/issue-284/D-002-conflicts-with-parent.gif
Output docs/demo-evidence/issue-284/D-002-conflicts-with-parent.webm

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

Type "jr issue edit FOO-100 --no-parent --parent BAR-200 2>&1"
Enter
Sleep 3s

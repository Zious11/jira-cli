Output docs/demo-evidence/issue-110-pr2/D-001-edit-help-new-flags.gif
Output docs/demo-evidence/issue-110-pr2/D-001-edit-help-new-flags.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 600
Set TypingSpeed 50ms
Set Theme "Catppuccin Mocha"
Set Padding 20

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-110-pr2'
Enter
Sleep 500ms
Show

Type './target/release/jr issue edit --help 2>&1 | head -35'
Enter
Sleep 3s

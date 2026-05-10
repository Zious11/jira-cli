Output docs/demo-evidence/issue-110-pr2/D-007-no-regression.gif
Output docs/demo-evidence/issue-110-pr2/D-007-no-regression.webm

Require cargo

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

Type 'cargo test --lib 2>&1 | tail -5'
Enter
Sleep 60s

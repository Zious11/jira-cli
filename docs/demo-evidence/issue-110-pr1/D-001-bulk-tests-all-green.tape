Output docs/demo-evidence/issue-110-pr1/D-001-bulk-tests-all-green.gif
Output docs/demo-evidence/issue-110-pr1/D-001-bulk-tests-all-green.webm

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
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-110-pr1'
Enter
Sleep 500ms
Show

Type 'cargo test --test issue_bulk 2>&1 | tail -15'
Enter
Sleep 15s

Output docs/demo-evidence/issue-110-pr1/D-003-edit-help-multi-key.gif
Output docs/demo-evidence/issue-110-pr1/D-003-edit-help-multi-key.webm

Require bash

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
Type 'export PATH="$PWD/target/release:$PATH"'
Enter
Sleep 500ms
Show

Type 'jr issue edit --help 2>&1 | head -20'
Enter
Sleep 4s

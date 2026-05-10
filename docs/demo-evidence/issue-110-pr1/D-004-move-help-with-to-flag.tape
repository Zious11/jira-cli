Output docs/demo-evidence/issue-110-pr1/D-004-move-help-with-to-flag.gif
Output docs/demo-evidence/issue-110-pr1/D-004-move-help-with-to-flag.webm

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

Type 'jr issue move --help 2>&1 | head -25'
Enter
Sleep 4s

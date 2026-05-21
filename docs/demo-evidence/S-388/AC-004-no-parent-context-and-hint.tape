Output docs/demo-evidence/S-388/AC-004-no-parent-context-and-hint.gif
Output docs/demo-evidence/S-388/AC-004-no-parent-context-and-hint.webm

Require bash

Set FontFamily "Menlo"
Set FontSize 14
Set Width 960
Set Height 440
Set Theme "Dracula"
Set Padding 18
Set Framerate 24
Set Shell "bash"

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-388"
Enter
Sleep 1s
Type "clear"
Enter
Sleep 500ms
Show

Type "# AC-4: --no-parent on subtask -> context sentence + CROSS_HIERARCHY_HINT, no fake endpoint"
Enter
Sleep 600ms

Type "bash docs/demo-evidence/S-388/run-demo.sh no_parent_subtask 19104 --no-input issue edit TEST-NP --no-parent"
Enter
Sleep 6s

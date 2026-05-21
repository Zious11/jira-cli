Output docs/demo-evidence/S-388/AC-001-cross-hierarchy-std-to-subtask.gif
Output docs/demo-evidence/S-388/AC-001-cross-hierarchy-std-to-subtask.webm

Require bash

Set FontFamily "Menlo"
Set FontSize 14
Set Width 900
Set Height 360
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

Type "# AC-1: cross-hierarchy: standard issue -> Sub-task type (BC-3.4.010)"
Enter
Sleep 600ms

Type "bash docs/demo-evidence/S-388/run-demo.sh cross_std_to_subtask 19101 --no-input issue edit TEST-1 --type Sub-task"
Enter
Sleep 6s

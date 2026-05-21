Output docs/demo-evidence/S-388/AC-002-cross-hierarchy-subtask-to-std.gif
Output docs/demo-evidence/S-388/AC-002-cross-hierarchy-subtask-to-std.webm

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

Type "# AC-2: reverse cross-hierarchy: sub-task -> Task type (BC-3.4.010)"
Enter
Sleep 600ms

Type "bash docs/demo-evidence/S-388/run-demo.sh cross_subtask_to_std 19102 --no-input issue edit SUB-1 --type Task"
Enter
Sleep 6s

Output docs/demo-evidence/S-388/AC-005-indeterminate-raw-error.gif
Output docs/demo-evidence/S-388/AC-005-indeterminate-raw-error.webm

Require bash

Set FontFamily "Menlo"
Set FontSize 14
Set Width 900
Set Height 320
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

Type "# AC-5 (indeterminate): project-types 503 -> raw error only, no JRACLOUD-27893 (BC-3.4.011)"
Enter
Sleep 600ms

Type "bash docs/demo-evidence/S-388/run-demo.sh indeterminate 19105 --no-input issue edit TEST-4 --type Task"
Enter
Sleep 6s

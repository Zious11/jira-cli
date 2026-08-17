Output docs/demo-evidence/S-604-2/AC-010-AC-011-no-fields-exit-64.gif
Output docs/demo-evidence/S-604-2/AC-010-AC-011-no-fields-exit-64.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1100
Set Height 400
Set Theme "Dracula"
Set Padding 20

Hide
Type "export PATH=/Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-604-2/target/debug:$PATH"
Enter
Sleep 200ms
Type "export JR_AUTH_HEADER='Basic dGVzdDp0ZXN0'"
Enter
Sleep 200ms
Show

# AC-010: edit NAME input, no field flags -> exit 64 "no fields specified to update", ZERO HTTP
Type "jr component edit Backend --project FOO --no-input"
Enter
Sleep 2s

# AC-011: edit numeric input, no field flags -> exit 64 "no fields specified to update", ZERO HTTP (including zero confirming GET)
Type "jr component edit 10042 --no-input"
Enter
Sleep 2s

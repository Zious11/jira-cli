Output docs/demo-evidence/S-604-2/AC-006-empty-lead-exit-64.gif
Output docs/demo-evidence/S-604-2/AC-006-empty-lead-exit-64.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1100
Set Height 300
Set Theme "Dracula"
Set Padding 20

Hide
Type "export JR_AUTH_HEADER='Basic dGVzdDp0ZXN0'"
Enter
Sleep 200ms
Show

# AC-006: create --lead "" -> exit 64 app-level guard, zero POST calls
Type "jr component create --project FOO Backend --lead '' --no-input"
Enter
Sleep 2s

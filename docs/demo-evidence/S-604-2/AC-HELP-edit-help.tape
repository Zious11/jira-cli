Output docs/demo-evidence/S-604-2/AC-HELP-edit-help.gif
Output docs/demo-evidence/S-604-2/AC-HELP-edit-help.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1100
Set Height 400
Set Theme "Dracula"
Set Padding 20

Hide
Type "export JR_AUTH_HEADER='Basic dGVzdDp0ZXN0'"
Enter
Sleep 200ms
Show

# AC-HELP: jr component edit --help shows NAME_OR_ID positional and all edit flags
Type "jr component edit --help"
Enter
Sleep 2s

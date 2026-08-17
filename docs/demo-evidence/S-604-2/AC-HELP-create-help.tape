Output docs/demo-evidence/S-604-2/AC-HELP-create-help.gif
Output docs/demo-evidence/S-604-2/AC-HELP-create-help.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1100
Set Height 500
Set Theme "Dracula"
Set Padding 20

Hide
Type "export JR_AUTH_HEADER='Basic dGVzdDp0ZXN0'"
Enter
Sleep 200ms
Show

# AC-HELP: jr component create --help shows all flags including --assignee-type ValueEnum
Type "jr component create --help"
Enter
Sleep 2s

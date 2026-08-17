Output docs/demo-evidence/S-604-2/AC-005-bad-assignee-type-exit-2.gif
Output docs/demo-evidence/S-604-2/AC-005-bad-assignee-type-exit-2.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1000
Set Height 300
Set Theme "Dracula"
Set Padding 20

Hide
Type "export JR_AUTH_HEADER='Basic dGVzdDp0ZXN0'"
Enter
Sleep 200ms
Show

# AC-005: --assignee-type BOGUS -> clap exit 2 (ValueEnum rejection), zero HTTP (DEC-188)
Type "jr component create --project FOO Backend --assignee-type BOGUS"
Enter
Sleep 2s

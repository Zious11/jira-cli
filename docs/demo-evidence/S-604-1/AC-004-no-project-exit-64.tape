Output docs/demo-evidence/S-604-1/AC-004-no-project-exit-64.gif
Output docs/demo-evidence/S-604-1/AC-004-no-project-exit-64.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1000
Set Height 400
Set Theme "Dracula"
Set Padding 20

Hide
Type "export JR_BASE_URL=http://127.0.0.1:9999"
Enter
Sleep 200ms
Type "export JR_AUTH_HEADER='Basic dGVzdDp0ZXN0'"
Enter
Sleep 200ms
Show

# AC-004: no --project, no .jr.toml configured -> exit 64 with --project hint
Type "jr component list"
Enter
Sleep 2s

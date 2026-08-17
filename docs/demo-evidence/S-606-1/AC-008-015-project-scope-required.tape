Output docs/demo-evidence/S-606-1/AC-008-015-project-scope-required.gif
Output docs/demo-evidence/S-606-1/AC-008-015-project-scope-required.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 500
Set Theme "Dracula"
Set Padding 20

Hide
Type "export JR_BASE_URL=http://127.0.0.1:9999"
Enter
Sleep 200ms
Type "export JR_AUTH_HEADER='Basic dGVzdDp0ZXN0'"
Enter
Sleep 200ms
Type "mkdir -p /tmp/s606-demo-noproj && cd /tmp/s606-demo-noproj"
Enter
Sleep 200ms
Type "clear"
Enter
Show

# AC-008: `--component none` with no --project and no configured project ->
# exit 64 pre-flight -- `none` is NOT exempt from project-scoping despite
# needing zero resolver HTTP.
Type "jr --no-input issue list --component none"
Enter
Sleep 2s

Hide
Type "clear"
Enter
Show

# AC-015: bare `--component <NAME>` with no --project and no configured
# project -> exit 64 pre-flight, BEFORE attempting the resolver GET.
Type "jr --no-input issue list --component Backend"
Enter
Sleep 2s

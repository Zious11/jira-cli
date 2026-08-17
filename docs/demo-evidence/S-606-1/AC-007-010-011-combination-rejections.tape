Output docs/demo-evidence/S-606-1/AC-007-010-011-combination-rejections.gif
Output docs/demo-evidence/S-606-1/AC-007-010-011-combination-rejections.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 700
Set Theme "Dracula"
Set Padding 20

Hide
Type "export JR_BASE_URL=http://127.0.0.1:9999"
Enter
Sleep 200ms
Type "export JR_AUTH_HEADER='Basic dGVzdDp0ZXN0'"
Enter
Sleep 200ms
Type "clear"
Enter
Show

# AC-007: `--component none` combined with another --component value ->
# exit 64 pre-flight, ZERO HTTP (JR_BASE_URL points at a closed port; the
# guard fires before any connection attempt would even be made).
Type "jr --no-input issue list --project FOO --component none --component Backend"
Enter
Sleep 2s

Hide
Type "clear"
Enter
Show

# AC-010: repeated `all:` occurrences -> exit 64, "may only be specified once"
Type "jr --no-input issue list --project FOO --component all:X --component all:Y"
Enter
Sleep 2s

Hide
Type "clear"
Enter
Show

# AC-011: `all:` mixed with a bare value -> exit 64 pre-flight, zero HTTP
Type "jr --no-input issue list --project FOO --component all:Backend --component Frontend"
Enter
Sleep 2s

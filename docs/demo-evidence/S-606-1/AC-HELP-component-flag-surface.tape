Output docs/demo-evidence/S-606-1/AC-HELP-component-flag-surface.gif
Output docs/demo-evidence/S-606-1/AC-HELP-component-flag-surface.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1400
Set Height 500
Set Theme "Dracula"
Set Padding 20

# AC surface demo: `jr issue list --help` documents the new --component flag
# and all four documented forms (bare, not:, none, all:) verbatim.
# No network required -- clap renders help text entirely offline.
Type "jr issue list --help | grep -A1 -- '--component '"
Enter
Sleep 3s

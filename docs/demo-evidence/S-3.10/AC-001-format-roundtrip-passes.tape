Output AC001_format_roundtrip_passes.gif
Output AC001_format_roundtrip_passes.webm

Require cargo

Set FontFamily "Menlo"
Set FontSize 14
Set Width 900
Set Height 400
Set Theme "Dracula"
Set Padding 20

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.10"
Enter
Sleep 1s
Show

Type "cargo test --lib -- duration::proptests::format_roundtrip"
Enter
Sleep 15s

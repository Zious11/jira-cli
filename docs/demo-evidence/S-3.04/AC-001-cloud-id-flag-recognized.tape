Output docs/demo-evidence/S-3.04/AC-001-cloud-id-flag-recognized.gif
Output docs/demo-evidence/S-3.04/AC-001-cloud-id-flag-recognized.webm

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 600
Set FontFamily "Menlo"
Set Theme "TokyoNight"
Set Padding 16

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.04"
Enter
Sleep 500ms
Show

Type 'cargo run --release --quiet -- auth login --help 2>&1 | grep -A 6 "cloud-id"'
Enter
Sleep 15s

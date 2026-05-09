Output docs/demo-evidence/S-3.08/AC-002-fmt-clean.gif
Output docs/demo-evidence/S-3.08/AC-002-fmt-clean.webm

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 600
Set FontFamily "Menlo"
Set Theme "TokyoNight"
Set Padding 16

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.08"
Enter
Sleep 500ms
Show

Type "cargo +nightly fmt --all -- --check 2>&1 && echo 'fmt check: Exit 0 (clean)'"
Enter
Sleep 15s

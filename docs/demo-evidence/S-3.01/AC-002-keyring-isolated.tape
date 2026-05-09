Output docs/demo-evidence/S-3.01/AC-002-keyring-isolated.gif
Output docs/demo-evidence/S-3.01/AC-002-keyring-isolated.webm

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 600
Set FontFamily "Menlo"
Set Theme "TokyoNight"
Set Padding 16

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.01"
Enter
Sleep 500ms
Show

Type 'grep -nE "keyring::Entry" src/cli/auth/*.rs'
Enter
Sleep 3s

Type 'echo "Exit: $? (no direct keyring::Entry in cli/auth/* — all keychain access flows through src/api/auth.rs)"'
Enter
Sleep 2s

Sleep 2s

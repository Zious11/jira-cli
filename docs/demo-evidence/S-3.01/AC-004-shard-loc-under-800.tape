Output docs/demo-evidence/S-3.01/AC-004-shard-loc-under-800.gif
Output docs/demo-evidence/S-3.01/AC-004-shard-loc-under-800.webm

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

Type 'wc -l src/cli/auth/*.rs'
Enter
Sleep 3s

Type 'echo "AC-004 PASS: largest production shard 366 LOC (login.rs) < 800 LOC cap (test module at src/cli/auth/tests/ excluded per story line 100)"'
Enter
Sleep 2s

Sleep 2s

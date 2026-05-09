Output docs/demo-evidence/S-3.01/AC-001-all-tests-green.gif
Output docs/demo-evidence/S-3.01/AC-001-all-tests-green.webm

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

Type 'cargo test --lib 2>&1 | tail -5'
Enter
Sleep 120s

Type 'cargo test --test auth_output_json -- --skip test_auth_login_emits_json_when_output_json_set 2>&1 | tail -5'
Enter
Sleep 30s

Sleep 2s

Output docs/demo-evidence/S-MUTANTS-SCOPE-GUARDS-1/AC-004-guard3-success.gif
Output docs/demo-evidence/S-MUTANTS-SCOPE-GUARDS-1/AC-004-guard3-success.webm

Require cargo

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Dracula"
Set Padding 20

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-MUTANTS-SCOPE-GUARDS-1 && clear"
Enter
Sleep 1s
Show

Type "cargo test --test mutants_glob_existence 2>&1 | tail -15"
Enter
Sleep 30s

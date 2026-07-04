Output docs/demo-evidence/S-MUTANTS-SCOPE-GUARDS-1/AC-005-guard3-failure.gif
Output docs/demo-evidence/S-MUTANTS-SCOPE-GUARDS-1/AC-005-guard3-failure.webm

Require cargo

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 900
Set Height 450
Set Theme "Dracula"
Set Padding 20

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-MUTANTS-SCOPE-GUARDS-1 && clear"
Enter
Sleep 1s
Show

Type "cargo test --test mutants_glob_existence test_reject_nonexistent_examine_globs_entry 2>&1 | tail -12"
Enter
Sleep 30s

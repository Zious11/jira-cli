Output docs/demo-evidence/S-MUTANTS-SCOPE-GUARDS-1/AC-002-guard2-selftest.gif
Output docs/demo-evidence/S-MUTANTS-SCOPE-GUARDS-1/AC-002-guard2-selftest.webm

Require bash

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 900
Set Height 350
Set Theme "Dracula"
Set Padding 20

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-MUTANTS-SCOPE-GUARDS-1 && clear"
Enter
Sleep 1s
Show

Type "bash scripts/check-cargo-mutants-policy-citations.sh --self-test && echo 'EXIT: 0'"
Enter
Sleep 8s

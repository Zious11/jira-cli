Output docs/demo-evidence/S-MUTANTS-SCOPE-GUARDS-1/AC-006-ci-wiring.gif
Output docs/demo-evidence/S-MUTANTS-SCOPE-GUARDS-1/AC-006-ci-wiring.webm

Require bash

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

Type "grep -A 12 'spec-guard:' .github/workflows/ci.yml | head -15"
Enter
Sleep 3s

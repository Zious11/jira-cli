Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-006-ci-wiring.gif
Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-006-ci-wiring.webm

Require bash
Set FontFamily "Menlo"
Set FontSize 15
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 24
Set TypingSpeed 40ms
Set Shell "bash"

# AC-006: CI wiring — spec-guard job name updated; two new Guard 1 steps

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-BC-CITATION-GUARD-1"
Enter
Sleep 300ms
Show

Type "grep -n 'spec-guard:\\|name: Spec Guards' .github/workflows/ci.yml"
Enter
Sleep 1s

Type "sed -n '133,142p' .github/workflows/ci.yml"
Enter
Sleep 2s

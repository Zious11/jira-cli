Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-005-floor-declaration.gif
Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-005-floor-declaration.webm

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

# AC-005: FLOOR=231 script-scope declaration + coverage floor guard logic

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-BC-CITATION-GUARD-1"
Enter
Sleep 300ms
Show

Type "grep -n '^FLOOR=' scripts/check-bc-citation-symbols.sh"
Enter
Sleep 1s

Type "grep -n 'BC-CITE-COVERAGE-FLOOR\\|expected >= ' scripts/check-bc-citation-symbols.sh"
Enter
Sleep 1s

Type "grep -n 'CANONICAL_MODE=1' scripts/check-bc-citation-symbols.sh | grep -v '#'"
Enter
Sleep 2s

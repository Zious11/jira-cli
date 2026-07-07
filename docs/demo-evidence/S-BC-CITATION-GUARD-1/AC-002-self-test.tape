Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-002-self-test.gif
Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-002-self-test.webm

Require bash
Set FontFamily "Menlo"
Set FontSize 16
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 24
Set TypingSpeed 40ms
Set Shell "bash"

# AC-002: Self-test — runs 10 fixtures (A-K) and 5 post-fixture assertions
# Exit 0; emits "All self-test fixtures passed (10/10)"
Type "bash /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-BC-CITATION-GUARD-1/scripts/check-bc-citation-symbols.sh --self-test"
Enter
Sleep 8s
